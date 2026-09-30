//! A bounded reader of the emitted Part 21 subset, not a general STEP importer.
use crate::diagnostic::{Diagnostic, Location, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub input_bytes: usize,
    pub entities: usize,
    pub references: usize,
    pub values: usize,
    pub nesting: usize,
    pub aggregate_items: usize,
    pub string_bytes: usize,
    pub expanded_items: usize,
    pub output_commands: usize,
    pub bundle_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            input_bytes: 32 * 1024 * 1024,
            entities: 500_000,
            references: 4_000_000,
            values: 8_000_000,
            nesting: 64,
            aggregate_items: 1_000_000,
            string_bytes: 1024 * 1024,
            expanded_items: 1_000_000,
            output_commands: 2_000_000,
            bundle_bytes: 256 * 1024 * 1024,
        }
    }
}
impl Limits {
    pub fn check(&self) -> Result<()> {
        if self.nesting == 0
            || self.nesting > 128
            || [
                self.input_bytes,
                self.entities,
                self.references,
                self.values,
                self.aggregate_items,
                self.string_bytes,
                self.expanded_items,
                self.output_commands,
                self.bundle_bytes,
            ]
            .contains(&0)
        {
            return Err(Diagnostic::new(
                "parse",
                "LIMIT_CONFIGURATION",
                "Limits must be positive; nesting must be at most 128",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Value {
    Number(f64),
    String(String),
    Reference(u64),
    Symbol(String),
    Aggregate(Vec<Value>),
    Typed(Entity),
}
impl Value {
    pub fn string(&self) -> Option<&str> {
        if let Self::String(x) = self {
            Some(x)
        } else {
            None
        }
    }
    pub fn number(&self) -> Option<f64> {
        if let Self::Number(x) = self {
            Some(*x)
        } else {
            None
        }
    }
    pub fn reference(&self) -> Option<u64> {
        if let Self::Reference(x) = self {
            Some(*x)
        } else {
            None
        }
    }
    pub fn symbol(&self) -> Option<&str> {
        if let Self::Symbol(x) = self {
            Some(x)
        } else {
            None
        }
    }
    pub fn aggregate(&self) -> Option<&[Value]> {
        if let Self::Aggregate(x) = self {
            Some(x)
        } else {
            None
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entity {
    pub name: String,
    pub args: Vec<Value>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Record {
    pub id: u64,
    pub parts: Vec<Entity>,
    pub location: Location,
}
#[derive(Debug)]
pub struct Document {
    pub records: BTreeMap<u64, Record>,
    pub header: Vec<Entity>,
    pub schema: String,
    pub input_sha256: String,
    pub input_bytes: usize,
    pub reference_count: usize,
    pub value_count: usize,
    types: BTreeMap<String, Vec<u64>>,
}
impl Document {
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = (&'a Record, &'a Entity)> {
        self.types
            .get(name)
            .into_iter()
            .flatten()
            .filter_map(move |id| {
                self.records
                    .get(id)
                    .and_then(|r| r.parts.iter().find(|e| e.name == name).map(|e| (r, e)))
            })
    }
    pub fn entity(&self, id: u64, name: &str) -> Result<&Entity> {
        let r = self
            .records
            .get(&id)
            .ok_or_else(|| Diagnostic::new("profile", "REFERENCE", format!("Missing #{id}")))?;
        r.parts.iter().find(|e| e.name == name).ok_or_else(|| {
            Diagnostic::new(
                "profile",
                "REFERENCE_TYPE",
                format!("Expected {name} at #{id}"),
            )
            .at(r.location.clone())
        })
    }
}
struct Parser<'a> {
    text: &'a str,
    at: usize,
    limits: &'a Limits,
    depth: usize,
    values: usize,
    refs: Vec<(u64, Location)>,
    record: Option<u64>,
    lines: Vec<usize>,
}
impl Parser<'_> {
    fn location(&self) -> Location {
        let i = self
            .lines
            .partition_point(|x| *x <= self.at)
            .saturating_sub(1);
        Location {
            byte_offset: self.at,
            line: i + 1,
            byte_column: self.at - self.lines[i] + 1,
            record: self.record,
        }
    }
    fn error(&self, code: &str, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new("parse", code, message).at(self.location())
    }
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }
    fn white(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.at += 1;
        }
    }
    fn take(&mut self, s: &str) -> Result<()> {
        self.white();
        if !self.text[self.at..].starts_with(s) {
            return Err(self.error("SYNTAX", format!("Expected {s}")));
        }
        self.at += s.len();
        Ok(())
    }
    fn ident(&mut self) -> Result<String> {
        self.white();
        let start = self.at;
        if !self
            .peek()
            .is_some_and(|b| b.is_ascii_uppercase() || b == b'_')
        {
            return Err(self.error("SYNTAX", "Expected entity identifier"));
        }
        self.at += 1;
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            self.at += 1;
        }
        Ok(self.text[start..self.at].into())
    }
    fn id(&mut self) -> Result<u64> {
        self.take("#")?;
        let start = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
        }
        let raw = &self.text[start..self.at];
        let n = raw
            .parse::<u64>()
            .map_err(|_| self.error("IDENTITY", "Invalid record/reference number"))?;
        if raw.starts_with('0') || n > 9_007_199_254_740_991 {
            return Err(self.error(
                "IDENTITY",
                "Record/reference must be a positive safe integer",
            ));
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<String> {
        self.take("'")?;
        let mut result = String::new();
        loop {
            let b = self
                .peek()
                .ok_or_else(|| self.error("STRING", "Unterminated string"))?;
            if b == b'\'' {
                self.at += 1;
                if self.peek() == Some(b'\'') {
                    self.at += 1;
                    result.push('\'');
                } else {
                    break;
                }
            } else if b == b'\\' {
                if self.text[self.at..].starts_with("\\\\") {
                    self.at += 2;
                    result.push('\\');
                } else if self.text[self.at..].starts_with("\\X2\\") {
                    self.at += 4;
                    let start = self.at;
                    let length = self.text[self.at..]
                        .find("\\X0\\")
                        .ok_or_else(|| self.error("STRING_ESCAPE", "Unterminated X2 escape"))?;
                    if length == 0 || length % 4 != 0 || length > self.limits.string_bytes {
                        return Err(self.error("STRING_ESCAPE", "Invalid X2 length"));
                    }
                    let hex = &self.text[start..start + length];
                    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err(self.error("STRING_ESCAPE", "Invalid X2 hex"));
                    }
                    let mut codes = Vec::with_capacity(length / 4);
                    for i in (0..length).step_by(4) {
                        codes
                            .push(u16::from_str_radix(&hex[i..i + 4], 16).map_err(|_| {
                                self.error("STRING_ESCAPE", "Invalid X2 code unit")
                            })?);
                    }
                    result
                        .push_str(&String::from_utf16(&codes).map_err(|_| {
                            self.error("STRING_ESCAPE", "Unpaired UTF16 surrogate")
                        })?);
                    self.at += length + 4;
                } else {
                    return Err(self.error("STRING_ESCAPE", "Unsupported STEP string escape"));
                }
            } else {
                let c = self.text[self.at..]
                    .chars()
                    .next()
                    .ok_or_else(|| self.error("STRING", "Invalid string"))?;
                if c.is_control() {
                    return Err(self.error("STRING", "Unescaped control character"));
                }
                result.push(c);
                self.at += c.len_utf8();
            }
            if result.len() > self.limits.string_bytes {
                return Err(self.error("STRING_LIMIT", "Decoded string exceeds limit"));
            }
        }
        Ok(result)
    }
    fn list(&mut self) -> Result<Vec<Value>> {
        self.depth += 1;
        if self.depth > self.limits.nesting {
            return Err(self.error("DEPTH_LIMIT", "Nesting limit exceeded"));
        }
        self.take("(")?;
        self.white();
        let mut v = Vec::new();
        if self.peek() != Some(b')') {
            loop {
                if v.len() >= self.limits.aggregate_items {
                    return Err(self.error("AGGREGATE_LIMIT", "Aggregate limit exceeded"));
                }
                v.push(self.value()?);
                self.white();
                if self.peek() != Some(b',') {
                    break;
                }
                self.at += 1;
            }
        }
        self.take(")")?;
        self.depth -= 1;
        Ok(v)
    }
    fn entity(&mut self) -> Result<Entity> {
        let name = self.ident()?;
        let args = self.list()?;
        Ok(Entity { name, args })
    }
    fn value(&mut self) -> Result<Value> {
        self.values += 1;
        if self.values > self.limits.values {
            return Err(self.error("VALUE_LIMIT", "Value limit exceeded"));
        }
        self.white();
        match self.peek() {
            Some(b'(') => Ok(Value::Aggregate(self.list()?)),
            Some(b'\'') => Ok(Value::String(self.string()?)),
            Some(b'#') => {
                if self.refs.len() >= self.limits.references {
                    return Err(self.error("REFERENCE_LIMIT", "Reference limit exceeded"));
                }
                let location = self.location();
                let id = self.id()?;
                self.refs.push((id, location));
                Ok(Value::Reference(id))
            }
            Some(b'$' | b'*') => {
                let s = self.text[self.at..self.at + 1].to_owned();
                self.at += 1;
                Ok(Value::Symbol(s))
            }
            Some(b'A'..=b'Z' | b'_') => Ok(Value::Typed(self.entity()?)),
            Some(b'.')
                if self
                    .text
                    .as_bytes()
                    .get(self.at + 1)
                    .is_some_and(|b| b.is_ascii_uppercase()) =>
            {
                let start = self.at;
                self.at += 1;
                self.ident()?;
                self.take(".")?;
                Ok(Value::Symbol(self.text[start..self.at].into()))
            }
            Some(b'0'..=b'9' | b'+' | b'-' | b'.') => {
                let start = self.at;
                if matches!(self.peek(), Some(b'+' | b'-')) {
                    self.at += 1;
                }
                let mut digits = 0;
                while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                    self.at += 1;
                    digits += 1;
                }
                if self.peek() == Some(b'.') {
                    self.at += 1;
                    while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                        self.at += 1;
                        digits += 1;
                    }
                }
                if digits == 0 {
                    return Err(self.error("NUMBER", "Missing number digits"));
                }
                if self.peek() == Some(b'E') {
                    self.at += 1;
                    if matches!(self.peek(), Some(b'+' | b'-')) {
                        self.at += 1;
                    }
                    let exp = self.at;
                    while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                        self.at += 1;
                    }
                    if exp == self.at {
                        return Err(self.error("NUMBER", "Missing exponent digits"));
                    }
                }
                let n = self.text[start..self.at]
                    .parse::<f64>()
                    .map_err(|_| self.error("NUMBER", "Invalid number"))?;
                if !n.is_finite() {
                    return Err(self.error("NUMBER", "Nonfinite number"));
                }
                Ok(Value::Number(n))
            }
            _ => Err(self.error("SYNTAX", "Expected attribute value")),
        }
    }
}
fn strings(value: Option<&Value>) -> bool {
    value
        .and_then(Value::aggregate)
        .is_some_and(|v| !v.is_empty() && v.iter().all(|x| x.string().is_some()))
}
pub fn parse(text: &str, limits: &Limits) -> Result<Document> {
    limits.check()?;
    if text.len() > limits.input_bytes {
        return Err(Diagnostic::new(
            "parse",
            "INPUT_SIZE",
            "Input exceeds byte limit",
        ));
    }
    let mut lines = vec![0];
    lines.extend(
        text.bytes()
            .enumerate()
            .filter_map(|(i, b)| if b == b'\n' { Some(i + 1) } else { None }),
    );
    let mut p = Parser {
        text,
        at: 0,
        limits,
        depth: 0,
        values: 0,
        refs: Vec::new(),
        record: None,
        lines,
    };
    p.take("ISO-10303-21;")?;
    p.take("HEADER;")?;
    let mut header = Vec::new();
    for name in ["FILE_DESCRIPTION", "FILE_NAME", "FILE_SCHEMA"] {
        let e = p.entity()?;
        if e.name != name {
            return Err(p.error("HEADER", format!("Expected {name}")));
        }
        p.take(";")?;
        header.push(e);
    }
    let d = &header[0].args;
    let n = &header[1].args;
    let s = &header[2].args;
    if d.len() != 2
        || !strings(d.first())
        || d[1].string().is_none()
        || n.len() != 7
        || ![0, 1, 4, 5, 6].iter().all(|i| n[*i].string().is_some())
        || !strings(n.get(2))
        || !strings(n.get(3))
        || s.len() != 1
        || !strings(s.first())
        || s[0].aggregate().map_or(0, |v| v.len()) != 1
    {
        return Err(p.error("HEADER", "Invalid header attributes"));
    }
    let schema = s[0]
        .aggregate()
        .and_then(|v| v.first())
        .and_then(Value::string)
        .ok_or_else(|| p.error("HEADER", "Missing schema"))?
        .to_owned();
    p.take("ENDSEC;")?;
    p.take("DATA;")?;
    let mut records = BTreeMap::new();
    let mut types: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    loop {
        p.white();
        if p.text[p.at..].starts_with("ENDSEC;") {
            break;
        }
        if records.len() >= limits.entities {
            return Err(p.error("ENTITY_LIMIT", "Entity limit exceeded"));
        }
        let mut location = p.location();
        let id = p.id()?;
        p.record = Some(id);
        location.record = Some(id);
        p.take("=")?;
        p.white();
        let mut parts = Vec::new();
        if p.peek() == Some(b'(') {
            p.at += 1;
            loop {
                p.white();
                if p.peek() == Some(b')') {
                    p.at += 1;
                    break;
                }
                if parts.len() >= limits.aggregate_items {
                    return Err(p.error("AGGREGATE_LIMIT", "Component limit exceeded"));
                }
                parts.push(p.entity()?);
            }
        } else {
            parts.push(p.entity()?);
        }
        p.take(";")?;
        let names: BTreeSet<_> = parts.iter().map(|e| &e.name).collect();
        if parts.is_empty() || names.len() != parts.len() {
            return Err(p.error("COMPONENTS", "Empty/duplicate complex components"));
        }
        if records.contains_key(&id) {
            return Err(p.error("IDENTITY", "Duplicate record ID"));
        }
        for e in &parts {
            types.entry(e.name.clone()).or_default().push(id);
        }
        records.insert(
            id,
            Record {
                id,
                parts,
                location,
            },
        );
    }
    p.record = None;
    p.take("ENDSEC;")?;
    p.take("END-ISO-10303-21;")?;
    p.white();
    if p.at != text.len() {
        return Err(p.error("ENVELOPE", "Trailing content"));
    }
    for (id, location) in &p.refs {
        if !records.contains_key(id) {
            return Err(
                Diagnostic::new("parse", "MISSING_REFERENCE", format!("Missing #{id}"))
                    .at(location.clone()),
            );
        }
    }
    Ok(Document {
        records,
        header,
        schema,
        input_sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
        input_bytes: text.len(),
        reference_count: p.refs.len(),
        value_count: p.values,
        types,
    })
}
