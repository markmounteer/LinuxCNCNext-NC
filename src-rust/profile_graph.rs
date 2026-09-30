//! Closed executable graph vocabulary. Shared geometry/process definitions are
//! allowed; ordered operation/toolpath uses are unique and must all be consumed.
use crate::{
    part21::{Document, Entity, Value},
    shape, Diagnostic, Result,
};
use std::collections::{BTreeMap, BTreeSet};

const OPERATIONS: [&str; 2] = ["TURNING_TYPE_OPERATION", "MILLING_TYPE_OPERATION"];
const METHODS: [&str; 8] = [
    "MACHINING_WORKPLAN",
    "MACHINING_WORKINGSTEP",
    "TURNING_TYPE_OPERATION",
    "MILLING_TYPE_OPERATION",
    "MACHINING_TOOLPATH",
    "MACHINING_TECHNOLOGY",
    "MACHINING_FUNCTIONS",
    "MACHINING_FEATURE_PROCESS",
];
const AUDITED_RELATIONSHIPS: [&str; 5] = [
    "MACHINING_PROCESS_SEQUENCE_RELATIONSHIP",
    "MACHINING_OPERATION_RELATIONSHIP",
    "MACHINING_TOOLPATH_SEQUENCE_RELATIONSHIP",
    "MACHINING_TECHNOLOGY_RELATIONSHIP",
    "MACHINING_FUNCTIONS_RELATIONSHIP",
];
#[derive(Clone, Debug)]
pub struct Property {
    pub property: u64,
    pub association: u64,
    pub representation: u64,
}
pub struct Graph<'a> {
    pub doc: &'a Document,
    pub revision: u32,
    pub milling: bool,
    pub workplan: u64,
    pub properties: BTreeMap<(u64, String), Property>,
    links: BTreeMap<(String, u64), Vec<u64>>,
    consumed_links: BTreeSet<u64>,
    visited: BTreeSet<u64>,
    used: BTreeSet<u64>,
    tools: BTreeMap<u64, Vec<u64>>,
}
impl<'a> Graph<'a> {
    pub fn fail(&self, id: u64, code: &str, message: impl Into<String>) -> Diagnostic {
        let error = Diagnostic::new("profile", code, message);
        match self.doc.records.get(&id) {
            Some(r) => error.at(r.location.clone()),
            None => error,
        }
    }
    pub fn singleton(&self, name: &str) -> Result<u64> {
        let mut values = self.doc.all(name);
        let first = values
            .next()
            .ok_or_else(|| Diagnostic::new("profile", "GRAPH", format!("Missing {name}")))?;
        if values.next().is_some() {
            return Err(self.fail(first.0.id, "GRAPH", format!("Expected one {name}")));
        }
        Ok(first.0.id)
    }
    pub fn single(&self, id: u64) -> Result<&Entity> {
        let r = self
            .doc
            .records
            .get(&id)
            .ok_or_else(|| self.fail(id, "REFERENCE", "Missing record"))?;
        if r.parts.len() != 1 {
            return Err(self.fail(id, "REFERENCE_TYPE", "Expected single-component entity"));
        }
        Ok(&r.parts[0])
    }
    pub fn property(&self, id: u64, name: &str, kind: &str) -> Result<&Entity> {
        let p = self
            .properties
            .get(&(id, name.into()))
            .ok_or_else(|| self.fail(id, "PROPERTY", format!("Missing {name}")))?;
        self.doc.entity(p.representation, kind)
    }
    pub fn has_property(&self, id: u64, name: &str) -> bool {
        self.properties.contains_key(&(id, name.into()))
    }
    pub fn text(&self, id: u64, name: &str) -> Result<&str> {
        let e = self.property(id, name, "REPRESENTATION")?;
        let values = e.args[1]
            .aggregate()
            .ok_or_else(|| self.fail(id, "PROPERTY", "Expected property item list"))?;
        if values.len() != 1 {
            return Err(self.fail(id, "PROPERTY", "Expected one descriptive item"));
        }
        let item = values[0]
            .reference()
            .ok_or_else(|| self.fail(id, "PROPERTY", "Expected item reference"))?;
        self.doc
            .entity(item, "DESCRIPTIVE_REPRESENTATION_ITEM")?
            .args[1]
            .string()
            .ok_or_else(|| self.fail(item, "PROPERTY", "Expected descriptive text"))
    }
    pub fn new(doc: &'a Document) -> Result<Self> {
        shape::validate(doc)?;
        let mut g = Self {
            doc,
            revision: 0,
            milling: false,
            workplan: 0,
            properties: BTreeMap::new(),
            links: BTreeMap::new(),
            consumed_links: BTreeSet::new(),
            visited: BTreeSet::new(),
            used: BTreeSet::new(),
            tools: BTreeMap::new(),
        };
        let mut associations = BTreeMap::new();
        for (r, e) in doc.all("ACTION_PROPERTY_REPRESENTATION") {
            if e.args[0].string() != Some("") || e.args[1].string() != Some("") {
                return Err(g.fail(
                    r.id,
                    "PROPERTY",
                    "Unexpected property association semantics",
                ));
            }
            let owner = e.args[2]
                .reference()
                .ok_or_else(|| g.fail(r.id, "PROPERTY", "Invalid property owner"))?;
            let rep = e.args[3]
                .reference()
                .ok_or_else(|| g.fail(r.id, "PROPERTY", "Invalid property representation"))?;
            if associations.insert(owner, (r.id, rep)).is_some() {
                return Err(g.fail(r.id, "PROPERTY", "Duplicate property association"));
            }
        }
        for (r, e) in doc.all("ACTION_PROPERTY") {
            let name = e.args[0]
                .string()
                .ok_or_else(|| g.fail(r.id, "PROPERTY", "Invalid name"))?;
            let owner = e.args[2]
                .reference()
                .ok_or_else(|| g.fail(r.id, "PROPERTY", "Invalid owner"))?;
            let (association, representation) = associations
                .remove(&r.id)
                .ok_or_else(|| g.fail(r.id, "PROPERTY", "Unrepresented property"))?;
            if g.properties
                .insert(
                    (owner, name.into()),
                    Property {
                        property: r.id,
                        association,
                        representation,
                    },
                )
                .is_some()
            {
                return Err(g.fail(r.id, "PROPERTY", "Duplicate property name on owner"));
            }
        }
        if !associations.is_empty() {
            return Err(Diagnostic::new(
                "profile",
                "PROPERTY",
                "Orphan property association",
            ));
        }
        g.workplan = g.singleton("MACHINING_WORKPLAN")?;
        g.singleton("MACHINING_PROJECT")?;
        let profile = g.text(g.workplan, "next-nc profile")?;
        let (milling, revision) = match profile {
            "next-nc/turning-toolpath/0.1" => (false, 1),
            "next-nc/milling-toolpath/0.1" => (true, 1),
            "next-nc/turning-toolpath/0.2" => (false, 2),
            "next-nc/milling-toolpath/0.2" => (true, 2),
            _ => return Err(g.fail(g.workplan, "PROFILE_VERSION", "Unsupported Next-NC profile")),
        };
        g.milling = milling;
        g.revision = revision;
        for (r, e) in doc
            .records
            .values()
            .flat_map(|r| r.parts.iter().map(move |e| (r, e)))
        {
            if e.name == "ACTION_PROPERTY" {
                let owner = e.args[2]
                    .reference()
                    .ok_or_else(|| g.fail(r.id, "PROPERTY", "Invalid owner"))?;
                let owner_type = g.single(owner)?.name.as_str();
                let name = e.args[0]
                    .string()
                    .ok_or_else(|| g.fail(r.id, "PROPERTY", "Invalid name"))?;
                let mut allowed = match owner_type {
                    "MACHINING_WORKPLAN" => vec!["next-nc profile", "next-nc coordinates"],
                    "TURNING_TYPE_OPERATION" | "MILLING_TYPE_OPERATION" => vec![
                        "next-nc tool offset",
                        "next-nc work offset",
                        "next-nc entry point",
                    ],
                    "MACHINING_TECHNOLOGY" => vec!["spindle", "feedrate", "feedrate reference"],
                    "MACHINING_FUNCTIONS" => vec!["coolant", "coolant type"],
                    "MACHINING_TOOLPATH" => vec![
                        "priority",
                        "trajectory type",
                        "direction",
                        "basic curve",
                        "speed profile",
                        "dwell",
                    ],
                    _ => vec![],
                };
                if revision == 2 {
                    match owner_type {
                        "MACHINING_WORKPLAN" => allowed.push("next-nc required capabilities"),
                        "TURNING_TYPE_OPERATION" | "MILLING_TYPE_OPERATION" => {
                            allowed.push("next-nc tolerance")
                        }
                        "MACHINING_TOOLPATH" => {
                            allowed.extend(["next-nc movement", "next-nc circular motion"])
                        }
                        _ => {}
                    }
                }
                if !allowed.contains(&name) || e.args[1].string() != Some("") {
                    return Err(g.fail(
                        r.id,
                        "UNSUPPORTED_PROPERTY",
                        format!("Unsupported {owner_type} property/semantics: {name}"),
                    ));
                }
                if ![
                    "spindle",
                    "feedrate",
                    "basic curve",
                    "speed profile",
                    "dwell",
                    "next-nc entry point",
                ]
                .contains(&name)
                {
                    let rep = g.property(owner, name, "REPRESENTATION")?;
                    let items = rep.args[1]
                        .aggregate()
                        .ok_or_else(|| g.fail(r.id, "PROPERTY", "Invalid items"))?;
                    if items.len() != 1 {
                        return Err(g.fail(r.id, "PROPERTY", "Expected one descriptive item"));
                    }
                    let item = g.doc.entity(
                        items[0]
                            .reference()
                            .ok_or_else(|| g.fail(r.id, "PROPERTY", "Invalid item"))?,
                        "DESCRIPTIVE_REPRESENTATION_ITEM",
                    )?;
                    if item.args[0].string() != Some(name) {
                        return Err(g.fail(
                            r.id,
                            "PROPERTY",
                            "Descriptive property label disagrees",
                        ));
                    }
                }
            }
            if METHODS.contains(&e.name.as_str())
                && (e.args[2].string() != Some("")
                    || e.args[3].string() != Some("")
                    || (e.name == "MACHINING_WORKINGSTEP"
                        && e.args[1].string() != Some("machining"))
                    || ((e.name == "MACHINING_WORKPLAN" || OPERATIONS.contains(&e.name.as_str()))
                        && e.args[1].string() != Some("")))
            {
                return Err(g.fail(
                    r.id,
                    "UNSUPPORTED_METHOD",
                    "Unexpected method classification, purpose or consequence",
                ));
            }
            let endpoints: Option<(&[&str], &[&str])> = match e.name.as_str() {
                "MACHINING_PROCESS_SEQUENCE_RELATIONSHIP" => {
                    Some((&["MACHINING_WORKPLAN"], &["MACHINING_WORKINGSTEP"]))
                }
                "MACHINING_OPERATION_RELATIONSHIP" => {
                    Some((&["MACHINING_WORKINGSTEP"], &OPERATIONS))
                }
                "MACHINING_TOOLPATH_SEQUENCE_RELATIONSHIP" => {
                    Some((&OPERATIONS, &["MACHINING_TOOLPATH"]))
                }
                "MACHINING_TECHNOLOGY_RELATIONSHIP" => Some((
                    &[
                        "TURNING_TYPE_OPERATION",
                        "MILLING_TYPE_OPERATION",
                        "MACHINING_TOOLPATH",
                    ],
                    &["MACHINING_TECHNOLOGY"],
                )),
                "MACHINING_FUNCTIONS_RELATIONSHIP" => Some((
                    &[
                        "TURNING_TYPE_OPERATION",
                        "MILLING_TYPE_OPERATION",
                        "MACHINING_TOOLPATH",
                    ],
                    &["MACHINING_FUNCTIONS"],
                )),
                "MACHINING_FEATURE_RELATIONSHIP" => {
                    Some((&["MACHINING_WORKINGSTEP"], &["MACHINING_FEATURE_PROCESS"]))
                }
                _ => None,
            };
            if let Some((from, to)) = endpoints {
                let a = e.args[2]
                    .reference()
                    .ok_or_else(|| g.fail(r.id, "RELATIONSHIP", "Invalid relating method"))?;
                let b = e.args[3]
                    .reference()
                    .ok_or_else(|| g.fail(r.id, "RELATIONSHIP", "Invalid related method"))?;
                if e.args[0].string() != Some("")
                    || e.args[1].string() != Some("")
                    || !from.contains(&g.single(a)?.name.as_str())
                    || !to.contains(&g.single(b)?.name.as_str())
                {
                    return Err(g.fail(
                        r.id,
                        "RELATIONSHIP",
                        "Unexpected relationship endpoints or semantics",
                    ));
                }
                g.links.entry((e.name.clone(), a)).or_default().push(r.id);
            }
        }
        for (r, e) in doc.all("MACHINING_TOOL") {
            let owners = e.args[2]
                .aggregate()
                .ok_or_else(|| g.fail(r.id, "TOOL", "Missing tool use list"))?;
            let mut unique = BTreeSet::new();
            for value in owners {
                let owner = value
                    .reference()
                    .ok_or_else(|| g.fail(r.id, "TOOL", "Invalid tool use"))?;
                if !OPERATIONS.contains(&g.single(owner)?.name.as_str()) || !unique.insert(owner) {
                    return Err(g.fail(r.id, "TOOL", "Duplicate or invalid tool owner"));
                }
                g.tools.entry(owner).or_default().push(r.id);
            }
        }
        let mut names_by_owner: BTreeMap<u64, Vec<&str>> = BTreeMap::new();
        for (owner, name) in g.properties.keys() {
            names_by_owner
                .entry(*owner)
                .or_default()
                .push(name.as_str());
        }
        for (r, e) in doc.all("MACHINING_TOOLPATH") {
            let names = names_by_owner
                .get(&r.id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let dwell = e.args[1].string() == Some("feedstop");
            if (dwell
                && (names.len() != if revision == 2 { 3 } else { 2 }
                    || !names.contains(&"priority")
                    || !names.contains(&"dwell")))
                || (!dwell && names.contains(&"dwell"))
            {
                return Err(g.fail(
                    r.id,
                    "AMBIGUOUS_PATH",
                    "Conflicting dwell/motion properties",
                ));
            }
        }
        for (r, _) in doc.all("MACHINING_FUNCTIONS") {
            if g.text(r.id, "coolant")? == "coolant off" && g.has_property(r.id, "coolant type") {
                return Err(g.fail(r.id, "AMBIGUOUS_COOLANT", "Coolant off with coolant type"));
            }
        }
        for (r, e) in doc.all("MACHINING_TECHNOLOGY_RELATIONSHIP") {
            let owner = e.args[2]
                .reference()
                .ok_or_else(|| g.fail(r.id, "RELATIONSHIP", "Invalid owner"))?;
            let tech = e.args[3]
                .reference()
                .ok_or_else(|| g.fail(r.id, "RELATIONSHIP", "Invalid technology"))?;
            if OPERATIONS.contains(&g.single(owner)?.name.as_str())
                && g.has_property(tech, "feedrate")
            {
                return Err(g.fail(
                    r.id,
                    "AMBIGUOUS_FEED",
                    "Operation initial state carries unused cutting feed",
                ));
            }
        }
        Ok(g)
    }
    pub fn visit(&mut self, id: u64, name: &str) -> Result<&'a Entity> {
        if !self.visited.insert(id) {
            return Err(self.fail(id, "REPEATED_USE", format!("Repeated {name}")));
        }
        self.doc.entity(id, name)
    }
    pub fn use_definition(&mut self, id: u64, name: &str) -> Result<&'a Entity> {
        self.used.insert(id);
        self.doc.entity(id, name)
    }
    pub fn targets(&mut self, kind: &str, owner: u64) -> Result<Vec<(u64, u64)>> {
        let links = self
            .links
            .get(&(kind.into(), owner))
            .cloned()
            .unwrap_or_default();
        let mut result = Vec::new();
        for id in links {
            let e = self.doc.entity(id, kind)?;
            let target = e.args[3]
                .reference()
                .ok_or_else(|| self.fail(id, "RELATIONSHIP", "Invalid target"))?;
            self.consumed_links.insert(id);
            result.push((target, id));
        }
        Ok(result)
    }
    pub fn target(&mut self, kind: &str, owner: u64) -> Result<u64> {
        let targets = self.targets(kind, owner)?;
        if targets.len() != 1 {
            return Err(self.fail(owner, "RELATIONSHIP", format!("Expected one {kind}")));
        }
        Ok(targets[0].0)
    }
    pub fn sequence(&mut self, kind: &str, owner: u64) -> Result<Vec<(u64, u64)>> {
        let mut targets = self.targets(kind, owner)?;
        let mut positions = BTreeMap::new();
        for (_, link) in &targets {
            let value = self
                .doc
                .entity(*link, kind)?
                .args
                .get(4)
                .and_then(Value::number)
                .ok_or_else(|| self.fail(*link, "SEQUENCE", "Missing sequence index"))?;
            if value.fract() != 0.0 || value < 1.0 || value > targets.len() as f64 {
                return Err(self.fail(*link, "SEQUENCE", "Non-contiguous sequence"));
            }
            positions.insert(*link, value as usize);
        }
        targets.sort_by_key(|(_, link)| positions[link]);
        if targets.is_empty()
            || targets
                .iter()
                .enumerate()
                .any(|(i, (_, link))| positions[link] != i + 1)
        {
            return Err(self.fail(
                owner,
                "SEQUENCE",
                "Empty, duplicate or non-contiguous sequence",
            ));
        }
        Ok(targets)
    }
    pub fn tool(&mut self, owner: u64) -> Result<u64> {
        let values = self
            .tools
            .get(&owner)
            .ok_or_else(|| self.fail(owner, "TOOL", "Missing tool"))?;
        if values.len() != 1 {
            return Err(self.fail(owner, "TOOL", "Expected one logical tool"));
        }
        let id = values[0];
        self.used.insert(id);
        Ok(id)
    }
    pub fn finish(&self) -> Result<()> {
        for kind in [
            "MACHINING_WORKINGSTEP",
            "TURNING_TYPE_OPERATION",
            "MILLING_TYPE_OPERATION",
            "MACHINING_TOOLPATH",
        ] {
            for (r, _) in self.doc.all(kind) {
                if !self.visited.contains(&r.id) {
                    return Err(self.fail(r.id, "ORPHAN", format!("Orphan {kind}")));
                }
            }
        }
        for kind in [
            "MACHINING_TOOL",
            "MACHINING_TECHNOLOGY",
            "MACHINING_FUNCTIONS",
        ] {
            for (r, _) in self.doc.all(kind) {
                if !self.used.contains(&r.id) {
                    return Err(self.fail(r.id, "ORPHAN", format!("Orphan {kind}")));
                }
            }
        }
        for kind in AUDITED_RELATIONSHIPS {
            for (r, _) in self.doc.all(kind) {
                if !self.consumed_links.contains(&r.id) {
                    return Err(self.fail(r.id, "ORPHAN", format!("Orphan {kind}")));
                }
            }
        }
        Ok(())
    }
}
