//! Complete source-profile decoding, before machine/setup plan binding.
//! A successful decode describes intent; it is never permission to execute.
use crate::{
    geometry, json,
    part21::{parse, Document, Entity, Limits, Value},
    profile_graph::Graph,
    units::Units,
    Diagnostic, Result,
};
use motion_command::{v2::Geometry, Plane, Rotation};
use serde::Serialize;
use serde_json::{json, Value as Json};
use std::{collections::BTreeSet, f64::consts::TAU};

pub const MOVEMENTS: [&str; 17] = [
    "unspecified",
    "rapid",
    "cutting",
    "finish-cutting",
    "lead-in",
    "lead-out",
    "link-transition",
    "link-direct",
    "ramp-helix",
    "ramp-profile",
    "ramp-zig-zag",
    "ramp",
    "plunge",
    "predrill",
    "extended",
    "reduced",
    "high-feed",
];
#[derive(Clone, Debug, Serialize)]
pub struct Fingerprint {
    pub schema: String,
    pub algorithm: String,
    pub value: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub profile: String,
    pub machine: String,
    pub units: String,
    pub entities: usize,
    pub sections: usize,
    pub paths: usize,
    pub expanded_items: usize,
    #[serde(rename = "programFingerprint")]
    pub fingerprint: Fingerprint,
    pub bounds: Json,
    #[serde(rename = "requiredCapabilities")]
    pub required_capabilities: Vec<String>,
    pub validation: &'static str,
}
#[derive(Clone, Debug, Serialize)]
pub struct Program {
    pub model: Json,
    pub report: Report,
    pub provenance: Json,
}
fn strings_equal(v: &Value, s: &str) -> bool {
    v.string() == Some(s)
}
fn refs(e: &Entity, index: usize) -> Result<Vec<u64>> {
    e.args
        .get(index)
        .and_then(Value::aggregate)
        .ok_or_else(|| Diagnostic::new("profile", "AGGREGATE", "Missing reference aggregate"))?
        .iter()
        .map(|v| {
            v.reference()
                .ok_or_else(|| Diagnostic::new("profile", "REFERENCE", "Expected reference"))
        })
        .collect()
}
fn reference(e: &Entity, index: usize) -> Result<u64> {
    e.args
        .get(index)
        .and_then(Value::reference)
        .ok_or_else(|| Diagnostic::new("profile", "REFERENCE", "Expected reference"))
}
fn one(values: Vec<u64>) -> Result<u64> {
    if values.len() != 1 {
        return Err(Diagnostic::new(
            "profile",
            "CARDINALITY",
            "Expected exactly one item",
        ));
    }
    Ok(values[0])
}
fn nonnegative(s: &str, min: u64) -> Result<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Diagnostic::new(
            "profile",
            "INTEGER",
            "Expected nonnegative decimal integer",
        ));
    }
    let n = s
        .parse::<u64>()
        .map_err(|_| Diagnostic::new("profile", "INTEGER", "Integer overflow"))?;
    if n < min || n > 9_007_199_254_740_991 {
        return Err(Diagnostic::new(
            "profile",
            "INTEGER",
            "Integer out of supported range",
        ));
    }
    Ok(n)
}
fn positive(n: f64) -> Result<f64> {
    if n.is_finite() && n > 0.0 {
        Ok(n)
    } else {
        Err(Diagnostic::new(
            "profile",
            "POSITIVE",
            "Expected positive finite value",
        ))
    }
}
fn three(values: &[Value]) -> Result<[f64; 3]> {
    if values.len() != 3 {
        return Err(Diagnostic::new(
            "geometry",
            "POINT",
            "Expected three coordinates",
        ));
    }
    let mut result = [0.0; 3];
    for (i, v) in values.iter().enumerate() {
        result[i] = v
            .number()
            .filter(|n| n.is_finite())
            .ok_or_else(|| Diagnostic::new("geometry", "POINT", "Nonfinite coordinate"))?;
    }
    Ok(result)
}
fn close(a: [f64; 3], b: [f64; 3], eps: f64) -> bool {
    a.into_iter().zip(b).all(|(x, y)| (x - y).abs() <= eps)
}
fn point_json(v: &Json) -> Result<[f64; 3]> {
    let a = v.as_array().filter(|a| a.len() == 3).ok_or_else(|| {
        Diagnostic::new("geometry", "POINT", "Expected three numeric coordinates")
    })?;
    let mut out = [0.0; 3];
    for (i, x) in a.iter().enumerate() {
        out[i] = x
            .as_f64()
            .filter(|x| x.is_finite())
            .ok_or_else(|| Diagnostic::new("geometry", "POINT", "Invalid coordinate"))?;
    }
    Ok(out)
}
fn field_number(v: &Json, key: &str) -> Result<f64> {
    v.get(key)
        .and_then(Json::as_f64)
        .filter(|n| n.is_finite())
        .ok_or_else(|| Diagnostic::new("profile", "NUMBER", format!("Invalid {key}")))
}
fn exact_keys(v: &Json, keys: &[&str]) -> Result<()> {
    let o = v
        .as_object()
        .ok_or_else(|| Diagnostic::new("profile", "FIELDS", "Expected object"))?;
    if o.len() != keys.len() || o.keys().any(|k| !keys.contains(&k.as_str())) {
        return Err(Diagnostic::new(
            "profile",
            "FIELDS",
            "Unknown or missing revision-2 fields",
        ));
    }
    Ok(())
}
struct Reader<'a> {
    g: Graph<'a>,
    units: Units<'a>,
    length: String,
    context: u64,
    limits: &'a Limits,
    expanded: usize,
    paths: usize,
    min: [f64; 3],
    max: [f64; 3],
}
impl Reader<'_> {
    fn location(&self, id: u64) -> Json {
        match self.g.doc.records.get(&id) {
            Some(r) => {
                json!({"record":format!("#{id}"),"sourceLine":r.location.line,"sourceByteColumn":r.location.byte_column,"byteOffset":r.location.byte_offset})
            }
            None => Json::Null,
        }
    }
    fn source_property(&self, id: u64, name: &str) -> Result<Json> {
        let Some(p) = self.g.properties.get(&(id, name.into())) else {
            return Ok(Json::Null);
        };
        let e = self.g.single(p.representation)?;
        let items = refs(e, 1)?
            .into_iter()
            .map(|id| self.location(id))
            .collect::<Vec<_>>();
        Ok(
            json!({"property":self.location(p.property),"association":self.location(p.association),"representation":self.location(p.representation),"items":items}),
        )
    }
    fn json_property(&self, id: u64, name: &str) -> Result<Json> {
        json::parse(self.g.text(id, name)?, self.limits)
            .map_err(|e| self.g.fail(id, &e.code, e.message))
    }
    fn bound(&mut self, p: [f64; 3]) {
        for (i, x) in p.into_iter().enumerate() {
            self.min[i] = self.min[i].min(x);
            self.max[i] = self.max[i].max(x);
        }
    }
    fn reserve(&mut self, n: usize) -> Result<()> {
        self.expanded = self.expanded.checked_add(n).ok_or_else(|| {
            Diagnostic::new("profile", "EXPANSION_LIMIT", "Source use count overflow")
        })?;
        if self.expanded > self.limits.expanded_items {
            return Err(Diagnostic::new(
                "profile",
                "EXPANSION_LIMIT",
                "Expanded source uses exceed limit",
            ));
        }
        Ok(())
    }
    fn point(&self, id: u64) -> Result<[f64; 3]> {
        let e = self.g.doc.entity(id, "CARTESIAN_POINT")?;
        let p = three(
            e.args[1]
                .aggregate()
                .ok_or_else(|| self.g.fail(id, "POINT", "Invalid point coordinates"))?,
        )?;
        if !self.g.milling && p[1] != 0.0 {
            return Err(self.g.fail(id, "POINT", "XZ lathe requires Y=0"));
        }
        Ok(p)
    }
    fn vector(&self, id: u64) -> Result<[f64; 3]> {
        let e = self.g.doc.entity(id, "DIRECTION")?;
        let v = three(
            e.args[1]
                .aggregate()
                .ok_or_else(|| self.g.fail(id, "UNIT_DIRECTION", "Invalid direction"))?,
        )?;
        if (v[0].hypot(v[1]).hypot(v[2]) - 1.0).abs() >= 1e-9 {
            return Err(self
                .g
                .fail(id, "UNIT_DIRECTION", "Direction must have unit length"));
        }
        Ok(v)
    }
    fn geometry_item(&self, id: u64, name: &str) -> Result<u64> {
        let e = self.g.property(id, name, "REPRESENTATION")?;
        if reference(e, 2)? != self.context {
            return Err(self.g.fail(
                id,
                "GEOMETRY_CONTEXT",
                "Geometry uses a different unit context",
            ));
        }
        one(refs(e, 1)?)
    }
    fn state(&mut self, id: u64) -> Result<(Json, Json)> {
        let tech = self.g.target("MACHINING_TECHNOLOGY_RELATIONSHIP", id)?;
        let e = self.g.use_definition(tech, "MACHINING_TECHNOLOGY")?;
        let process = if self.g.milling { "milling" } else { "turning" };
        if !strings_equal(&e.args[1], process)
            || self.g.text(tech, "feedrate reference")? != "tool center point"
        {
            return Err(self
                .g
                .fail(tech, "TECHNOLOGY", "Unsupported technology/feed reference"));
        }
        let sr = self
            .g
            .property(tech, "spindle", "MACHINING_SPINDLE_SPEED_REPRESENTATION")?;
        let css = sr.args[0].string() == Some("cutting speed");
        if !(css || sr.args[0].string() == Some("spindle speed")) || css && self.g.milling {
            return Err(self
                .g
                .fail(tech, "SPINDLE_MODE", "Unsupported spindle mode for machine"));
        }
        let measures = refs(sr, 1)?;
        if measures.len() != if css { 2 } else { 1 } {
            return Err(self
                .g
                .fail(tech, "SPINDLE_MEASURES", "Wrong number of spindle measures"));
        }
        let speed = self.units.measure(
            measures[0],
            &if css {
                format!("{}/minute", self.length)
            } else {
                "revolution/minute".into()
            },
            "NUMERIC_MEASURE",
        )?;
        let mut spindle = json!({"mode":if css{"css"}else{"rpm"},"speed":positive(speed.abs())?,"clockwise":speed<0.0});
        if css {
            spindle["maximumRPM"] = json!(positive(self.units.measure(
                measures[1],
                "revolution/minute",
                "NUMERIC_MEASURE"
            )?)?);
        }
        let feed = if self.g.has_property(tech, "feedrate") {
            let fr = self
                .g
                .property(tech, "feedrate", "MACHINING_FEED_SPEED_REPRESENTATION")?;
            let rev = fr.args[0].string() == Some("feed per revolution");
            if !rev && fr.args[0].string() != Some("feed speed") {
                return Err(self.g.fail(tech, "FEED_MODE", "Unsupported feed mode"));
            }
            let unit = format!(
                "{}/{}",
                self.length,
                if rev { "revolution" } else { "minute" }
            );
            json!({"value":positive(self.units.measure(one(refs(fr,1)?)?,&unit,"NUMERIC_MEASURE")?)?,"mode":if rev{"perRevolution"}else{"perMinute"}})
        } else {
            Json::Null
        };
        let functions = self.g.target("MACHINING_FUNCTIONS_RELATIONSHIP", id)?;
        let e = self.g.use_definition(functions, "MACHINING_FUNCTIONS")?;
        if !strings_equal(&e.args[1], process) {
            return Err(self
                .g
                .fail(functions, "FUNCTIONS", "Unsupported machine functions"));
        }
        let coolant = match self.g.text(functions, "coolant")? {
            "coolant off" => "off",
            "coolant on" => self.g.text(functions, "coolant type")?,
            _ => return Err(self.g.fail(functions, "COOLANT", "Invalid coolant state")),
        };
        if !["off", "flood", "mist", "through tool"].contains(&coolant) {
            return Err(self
                .g
                .fail(functions, "COOLANT", "Unsupported coolant type"));
        }
        // Preserve the legacy translator's explicit unsupported capability gate.
        if coolant == "through tool" {
            return Err(self.g.fail(
                functions,
                "COOLANT_CAPABILITY",
                "Through-tool coolant requires an implemented target mapping",
            ));
        }
        let state = json!({"spindle":spindle,"feed":feed,"coolant":coolant});
        let source = json!({"technology":self.location(tech),"functions":self.location(functions),"spindle":self.source_property(tech,"spindle")?,"feed":self.source_property(tech,"feedrate")?,"coolant":self.source_property(functions,"coolant")?,"coolantType":self.source_property(functions,"coolant type")?});
        Ok((state, source))
    }
    fn arc(&mut self, id: u64) -> Result<(Json, Json)> {
        self.reserve(1)?;
        let trim = self.g.doc.entity(id, "TRIMMED_CURVE")?;
        let circle_id = reference(trim, 1)?;
        let circle = self.g.doc.entity(circle_id, "CIRCLE")?;
        let axis_id = reference(circle, 1)?;
        let axis = self.g.doc.entity(axis_id, "AXIS2_PLACEMENT_3D")?;
        let center_id = reference(axis, 1)?;
        let normal_id = reference(axis, 2)?;
        let direction_id = reference(axis, 3)?;
        let center = self.point(center_id)?;
        let normal = self.vector(normal_id)?;
        let direction = self.vector(direction_id)?;
        let radius = positive(
            circle.args[2]
                .number()
                .ok_or_else(|| self.g.fail(circle_id, "ARC_RADIUS", "Invalid radius"))?,
        )?;
        let (plane, label, u, v, n, sign) = if normal == [0., 0., 1.] {
            (Plane::Xy, "XY", 0, 1, 2, 1.)
        } else if normal == [0., 1., 0.] {
            (Plane::Xz, "XZ", 0, 2, 1, -1.)
        } else if normal == [1., 0., 0.] {
            (Plane::Yz, "YZ", 1, 2, 0, 1.)
        } else {
            return Err(self
                .g
                .fail(normal_id, "ARC_PLANE", "Unsupported arc normal"));
        };
        if (!self.g.milling && plane != Plane::Xz) || direction[n] != 0.0 {
            return Err(self
                .g
                .fail(axis_id, "ARC_PLANE", "Invalid plane/reference direction"));
        }
        let clockwise = match trim.args[4].symbol() {
            Some(".T.") => false,
            Some(".F.") => true,
            _ => {
                return Err(self
                    .g
                    .fail(id, "ARC_DIRECTION", "Arc direction must be explicit"))
            }
        };
        let full = trim.args[5].symbol() == Some(".PARAMETER.");
        let first = trim.args[2]
            .aggregate()
            .and_then(|a| if a.len() == 1 { a.first() } else { None })
            .ok_or_else(|| self.g.fail(id, "ARC_TRIM", "Expected one first trim"))?;
        let last = trim.args[3]
            .aggregate()
            .and_then(|a| if a.len() == 1 { a.first() } else { None })
            .ok_or_else(|| self.g.fail(id, "ARC_TRIM", "Expected one last trim"))?;
        let (start, end, start_source, end_source) = if full {
            let param = |v: &Value, x: f64| matches!(v,Value::Typed(e)if e.name=="PARAMETER_VALUE"&&e.args.len()==1&&e.args[0].number()==Some(x));
            if !param(first, 0.0) || !param(last, TAU) {
                return Err(self
                    .g
                    .fail(id, "ARC_TRIM", "Unsupported full-circle parameters"));
            }
            let p = std::array::from_fn(|i| center[i] + radius * direction[i]);
            let source = json!({"derivation":"center + radius * referenceDirection","center":self.location(center_id),"radius":self.location(circle_id),"referenceDirection":self.location(direction_id)});
            (p, p, source.clone(), source)
        } else {
            if trim.args[5].symbol() != Some(".CARTESIAN.") {
                return Err(self.g.fail(id, "ARC_TRIM", "Unsupported trim mode"));
            }
            let a = first
                .reference()
                .ok_or_else(|| self.g.fail(id, "ARC_TRIM", "Invalid first point"))?;
            let b = last
                .reference()
                .ok_or_else(|| self.g.fail(id, "ARC_TRIM", "Invalid last point"))?;
            let start = self.point(a)?;
            let end = self.point(b)?;
            if start == end {
                return Err(self
                    .g
                    .fail(id, "ARC_TRIM", "Partial arc has coincident endpoints"));
            }
            (start, end, self.location(a), self.location(b))
        };
        let allowance = 1e-7_f64.max(radius * 1e-6);
        for p in [start, end] {
            let measured = (p[u] - center[u]).hypot(p[v] - center[v]);
            if p[n] != center[n] {
                return Err(self.g.fail(id, "ARC_PLANAR", "Legacy arc is not planar"));
            }
            if (measured - radius).abs() > allowance {
                return Err(self
                    .g
                    .fail(id, "ARC_RADIUS", "Endpoint radius mismatch")
                    .with("radius", radius)
                    .with("measuredRadius", measured)
                    .with("threshold", allowance));
            }
            self.bound(p);
        }
        if !close(
            direction,
            std::array::from_fn(|i| (start[i] - center[i]) / radius),
            1e-8,
        ) {
            return Err(self.g.fail(
                direction_id,
                "ARC_DIRECTION",
                "Reference direction disagrees with start",
            ));
        }
        let a = (sign * (start[v] - center[v])).atan2(start[u] - center[u]);
        let b = (sign * (end[v] - center[v])).atan2(end[u] - center[u]);
        let sweep = if full {
            TAU
        } else {
            (if clockwise { a - b } else { b - a }).rem_euclid(TAU)
        };
        // Match the legacy swept-bound convention, independently of serialization.
        for i in 0..4 {
            let angle = i as f64 * std::f64::consts::FRAC_PI_2;
            let along = (if clockwise { a - angle } else { angle - a }).rem_euclid(TAU);
            if along <= sweep + 1e-12 {
                let mut p = center;
                p[u] += radius * [1., 0., -1., 0.][i];
                p[v] += sign * radius * [0., 1., 0., -1.][i];
                self.bound(p);
            }
        }
        let mut path = json!({"kind":"arc","start":start,"end":end,"center":center,"radius":radius,"clockwise":clockwise,"fullCircle":full});
        if self.g.milling {
            path["plane"] = json!(label);
        }
        let source = json!({"circle":self.location(circle_id),"placement":self.location(axis_id),"center":self.location(center_id),"normal":self.location(normal_id),"referenceDirection":self.location(direction_id),"start":start_source,"end":end_source});
        Ok((path, source))
    }
    fn circular(&mut self, id: u64) -> Result<Json> {
        self.reserve(1)?;
        let value = self.json_property(id, "next-nc circular motion")?;
        exact_keys(
            &value,
            &[
                "start",
                "end",
                "center",
                "plane",
                "clockwise",
                "sweepRadians",
                "axialRise",
            ],
        )?;
        let start = point_json(&value["start"])?;
        let end = point_json(&value["end"])?;
        let center = point_json(&value["center"])?;
        for p in [start, end, center] {
            if p.iter().any(|x| x.abs() >= 1e15) || !self.g.milling && p[1] != 0.0 {
                return Err(self.g.fail(id, "POINT", "Invalid circular coordinates"));
            }
        }
        let plane = match value["plane"].as_str() {
            Some("XY") => Plane::Xy,
            Some("XZ") => Plane::Xz,
            Some("YZ") => Plane::Yz,
            _ => return Err(self.g.fail(id, "ARC_PLANE", "Unknown circular plane")),
        };
        if !self.g.milling && plane != Plane::Xz {
            return Err(self
                .g
                .fail(id, "ARC_PLANE", "Lathe circular plane must be XZ"));
        }
        let clockwise = value["clockwise"].as_bool().ok_or_else(|| {
            self.g
                .fail(id, "ARC_DIRECTION", "Explicit Boolean direction required")
        })?;
        let sweep = field_number(&value, "sweepRadians")?;
        let rise = field_number(&value, "axialRise")?;
        let (_, _, n) = geometry::basis(plane);
        if center[n] != start[n] || end[n] - start[n] != rise {
            return Err(self.g.fail(
                id,
                "ARC_RISE",
                "Source center height or signed rise disagrees",
            ));
        }
        let scale = if self.length == "inch" { 25.4 } else { 1.0 };
        let start_mm = start.map(|x| x * scale);
        let end_mm = end.map(|x| x * scale);
        let geom = Geometry::Circular {
            start: geometry::point(start_mm),
            end: geometry::point(end_mm),
            center: geometry::point(center.map(|x| x * scale)),
            plane,
            rotation: if clockwise {
                Rotation::Clockwise
            } else {
                Rotation::Counterclockwise
            },
            sweep_radians: sweep,
            axial_rise_mm: end_mm[n] - start_mm[n],
        };
        let m = geometry::validate_with_floor(geom, 1e-7 * scale)
            .map_err(|e| self.g.fail(id, &e.code, e.message))?;
        self.bound(m.minimum_mm.map(|x| x / scale));
        self.bound(m.maximum_mm.map(|x| x / scale));
        Ok(
            json!({"kind":"circular","start":start,"end":end,"center":center,"plane":value["plane"],"clockwise":clockwise,"sweepRadians":sweep,"axialRise":rise}),
        )
    }
    fn section(&mut self, ws: u64) -> Result<(Json, Json)> {
        let step = self.g.visit(ws, "MACHINING_WORKINGSTEP")?;
        let id = self.g.target("MACHINING_OPERATION_RELATIONSHIP", ws)?;
        let op = self.g.visit(
            id,
            if self.g.milling {
                "MILLING_TYPE_OPERATION"
            } else {
                "TURNING_TYPE_OPERATION"
            },
        )?;
        if step.args[0] != op.args[0] {
            return Err(self
                .g
                .fail(id, "OPERATION", "Workingstep and operation names differ"));
        }
        let name = op.args[0]
            .string()
            .ok_or_else(|| self.g.fail(id, "OPERATION", "Invalid operation name"))?;
        let tool_id = self.g.tool(id)?;
        let tool = self.g.doc.entity(tool_id, "MACHINING_TOOL")?;
        if !strings_equal(
            &self
                .g
                .doc
                .entity(reference(tool, 3)?, "ACTION_RESOURCE_TYPE")?
                .args[0],
            "cutting tool",
        ) {
            return Err(self.g.fail(tool_id, "TOOL", "Unsupported tool type"));
        }
        let number = nonnegative(
            tool.args[0]
                .string()
                .ok_or_else(|| self.g.fail(tool_id, "TOOL", "Invalid tool number"))?,
            1,
        )?;
        let offset = nonnegative(self.g.text(id, "next-nc tool offset")?, 0)?;
        let wcs = nonnegative(self.g.text(id, "next-nc work offset")?, 0)?;
        let (initial, initial_source) = self.state(id)?;
        let start = self.point(self.geometry_item(id, "next-nc entry point")?)?;
        self.bound(start);
        let mut section = json!({"name":name,"tool":{"number":number,"offset":offset,"description":tool.args[1].string()},"workOffset":wcs,"start":start,"initialSpindle":initial["spindle"],"initialCoolant":initial["coolant"],"paths":[]});
        if self.g.revision == 2 {
            let t = self.json_property(id, "next-nc tolerance")?;
            exact_keys(&t, &["value", "provenance"])?;
            match t["provenance"].as_str() {
                Some("missing") if t["value"].is_null() => {}
                Some("source-declared" | "fusion:operation:tolerance") => {
                    positive(field_number(&t, "value")?)?;
                }
                _ => {
                    return Err(self
                        .g
                        .fail(id, "TOLERANCE", "Invalid tolerance provenance/value"))
                }
            }
            section["tolerance"] = json!({"value":t["value"],"provenance":t["provenance"]});
        }
        let mut source = json!({"workingstep":self.location(ws),"operation":self.location(id),"tool":self.location(tool_id),"toolOffset":self.source_property(id,"next-nc tool offset")?,"workOffset":self.source_property(id,"next-nc work offset")?,"entry":self.source_property(id,"next-nc entry point")?,"process":initial_source,"paths":[]});
        if self.g.revision == 2 {
            source["tolerance"] = self.source_property(id, "next-nc tolerance")?;
        }
        let mut paths = Vec::new();
        let mut sources = Vec::new();
        let mut position = start;
        for (path_index, (pid, link)) in self
            .g
            .sequence("MACHINING_TOOLPATH_SEQUENCE_RELATIONSHIP", id)?
            .into_iter()
            .enumerate()
        {
            let result = self.path(pid, position).map_err(|mut e| {
                if e.source.is_none() {
                    e.source = self
                        .g
                        .doc
                        .records
                        .get(&pid)
                        .map(|r| Box::new(r.location.clone()));
                }
                e.with("operation", name).with("path", path_index + 1)
            })?;
            let (mut decoded, mut path_source, end) = result;
            position = end;
            path_source["sequenceRelationship"] = self.location(link);
            // This distinction is only in provenance, never a geometry edit.
            decoded
                .as_object_mut()
                .ok_or_else(|| self.g.fail(pid, "INTERNAL", "Invalid decoded path"))?;
            paths.push(decoded);
            sources.push(path_source);
            self.paths += 1;
        }
        section["paths"] = Json::Array(paths);
        source["paths"] = Json::Array(sources);
        Ok((section, source))
    }
    fn path(&mut self, id: u64, position: [f64; 3]) -> Result<(Json, Json, [f64; 3])> {
        let path = self.g.visit(id, "MACHINING_TOOLPATH")?;
        let (state, process_source) = self.state(id)?;
        if self.g.text(id, "priority")? != "required" {
            return Err(self
                .g
                .fail(id, "PRIORITY", "Only required paths are supported"));
        }
        let mut source = json!({"toolpath":self.location(id),"process":process_source});
        let mut decoded;
        let mut end = position;
        if path.args[1].string() == Some("feedstop") {
            self.reserve(1)?;
            if !state["feed"].is_null() {
                return Err(self
                    .g
                    .fail(id, "AMBIGUOUS_FEED", "Dwell cannot carry cutting feed"));
            }
            let rep = self.g.property(id, "dwell", "REPRESENTATION")?;
            let seconds = positive(self.units.measure(
                one(refs(rep, 1)?)?,
                "second",
                "TIME_MEASURE",
            )?)?;
            decoded = json!({"kind":"dwell","seconds":seconds});
            source["dwell"] = self.source_property(id, "dwell")?;
        } else {
            if path.args[1].string() != Some("cutter location trajectory")
                || self.g.text(id, "trajectory type")? != "trajectory path"
                || self.g.text(id, "direction")? != "beginning to end"
            {
                return Err(self
                    .g
                    .fail(id, "TRAJECTORY", "Unsupported trajectory semantics"));
            }
            let native = self.g.has_property(id, "next-nc circular motion");
            let rapid = self.g.has_property(id, "speed profile");
            if native {
                if self.g.revision != 2 || rapid || self.g.has_property(id, "basic curve") {
                    return Err(self.g.fail(
                        id,
                        "AMBIGUOUS_PATH",
                        "Conflicting native circular representation",
                    ));
                }
                decoded = self.circular(id)?;
                source["circular"] = self.source_property(id, "next-nc circular motion")?;
            } else {
                let curve = self.geometry_item(id, "basic curve")?;
                source["basicCurve"] = self.source_property(id, "basic curve")?;
                source["curve"] = self.location(curve);
                source["speedProfile"] = self.source_property(id, "speed profile")?;
                if rapid {
                    let sr = self.g.property(
                        id,
                        "speed profile",
                        "MACHINING_TOOLPATH_SPEED_PROFILE_REPRESENTATION",
                    )?;
                    let item = self
                        .g
                        .doc
                        .entity(one(refs(sr, 1)?)?, "DESCRIPTIVE_REPRESENTATION_ITEM")?;
                    if !strings_equal(&item.args[1], "rapid") {
                        return Err(self
                            .g
                            .fail(id, "SPEED_PROFILE", "Unsupported speed profile"));
                    }
                }
                if self.g.doc.entity(curve, "POLYLINE").is_ok() {
                    let vertices = refs(self.g.doc.entity(curve, "POLYLINE")?, 1)?;
                    self.reserve(vertices.len())?;
                    let points = vertices
                        .iter()
                        .map(|id| self.point(*id))
                        .collect::<Result<Vec<_>>>()?;
                    if points.len() < 2 {
                        return Err(self.g.fail(curve, "POLYLINE", "Polyline needs two points"));
                    }
                    for p in &points {
                        self.bound(*p);
                    }
                    decoded = json!({"kind":if rapid{"rapid"}else{"linear"},"points":points});
                    source["vertices"] = json!(vertices
                        .iter()
                        .map(|id| self.location(*id))
                        .collect::<Vec<_>>());
                } else {
                    if rapid {
                        return Err(self.g.fail(id, "RAPID_ARC", "Rapid arc unsupported"));
                    }
                    let (arc, arc_source) = self.arc(curve)?;
                    decoded = arc;
                    source["arc"] = arc_source;
                }
            }
            if rapid != state["feed"].is_null() {
                return Err(self
                    .g
                    .fail(id, "FEED", "Rapid/cutting feed state disagrees"));
            }
            let first = if decoded["points"].is_array() {
                point_json(&decoded["points"][0])?
            } else {
                point_json(&decoded["start"])?
            };
            if !close(position, first, 1e-9) {
                return Err(self
                    .g
                    .fail(
                        id,
                        "PATH_CONTINUITY",
                        "Path start differs from previous exit",
                    )
                    .with("previousExit", json!(position))
                    .with("sourceStart", json!(first))
                    .with("threshold", 1e-9));
            }
            end = if let Some(points) = decoded["points"].as_array() {
                point_json(
                    points
                        .last()
                        .ok_or_else(|| self.g.fail(id, "POLYLINE", "Missing last point"))?,
                )?
            } else {
                point_json(&decoded["end"])?
            };
        }
        if self.g.revision == 2 {
            let movement = self.g.text(id, "next-nc movement")?;
            if !MOVEMENTS.contains(&movement)
                || (decoded["kind"] != "dwell"
                    && movement != "unspecified"
                    && ((decoded["kind"] == "rapid") != (movement == "rapid")))
            {
                return Err(self.g.fail(
                    id,
                    "MOVEMENT",
                    "Movement purpose disagrees with motion/feed type",
                ));
            }
            decoded["movement"] = json!(movement);
            source["movement"] = self.source_property(id, "next-nc movement")?;
        }
        let object = decoded
            .as_object_mut()
            .ok_or_else(|| self.g.fail(id, "INTERNAL", "Invalid decoded path"))?;
        for key in ["spindle", "feed", "coolant"] {
            object.insert(key.into(), state[key].clone());
        }
        Ok((decoded, source, end))
    }
}
pub fn requirements(model: &Json) -> Result<Vec<String>> {
    let mut out: BTreeSet<String> = [
        "completion",
        "spindle",
        "tool-change",
        "tool-offset",
        "coolant",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let sections = model["sections"]
        .as_array()
        .ok_or_else(|| Diagnostic::new("profile", "MODEL", "Missing sections"))?;
    for s in sections {
        if s["initialSpindle"]["mode"] == "css" {
            out.insert("css".into());
        }
        for p in s["paths"]
            .as_array()
            .ok_or_else(|| Diagnostic::new("profile", "MODEL", "Missing paths"))?
        {
            out.insert(
                match p["kind"].as_str() {
                    Some("dwell") => "dwell",
                    Some("arc" | "circular") => "planar-arc",
                    _ => "linear",
                }
                .into(),
            );
            if p["axialRise"].as_f64().is_some_and(|x| x != 0.0) {
                out.insert("helix".into());
            }
            if p["sweepRadians"].as_f64().is_some_and(|x| x > TAU) {
                out.insert("multiple-turns".into());
            }
            if p["feed"]["mode"] == "perRevolution" {
                out.insert("feed-per-revolution".into());
            }
            if p["spindle"]["mode"] == "css" {
                out.insert("css".into());
            }
        }
    }
    Ok(out.into_iter().collect())
}
pub fn decode_document(doc: &Document, limits: &Limits) -> Result<Program> {
    let g = Graph::new(doc)?;
    let mut units = Units::new(doc, limits.nesting);
    let (context, length) = units.geometry_context()?;
    let workplan = g.workplan;
    let profile = g.text(workplan, "next-nc profile")?.to_owned();
    let revision = g.revision;
    let milling = g.milling;
    let convention = if milling {
        "WCS; XYZ Cartesian; fixed +Z tool axis; Fusion tool reference point"
    } else {
        "WCS; X radius; Y zero; Z axial; Fusion tool reference point"
    };
    if g.text(workplan, "next-nc coordinates")? != convention {
        return Err(g.fail(workplan, "COORDINATES", "Unsupported coordinate convention"));
    }
    let name = g.doc.entity(workplan, "MACHINING_WORKPLAN")?.args[0]
        .string()
        .ok_or_else(|| g.fail(workplan, "NAME", "Invalid workplan name"))?;
    let mut model = json!({"name":name,"units":length});
    if milling {
        model["machine"] = json!("mill");
    }
    model["sections"] = json!([]);
    let mut r = Reader {
        g,
        units,
        length: length.clone(),
        context,
        limits,
        expanded: 0,
        paths: 0,
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    if revision == 2 {
        model["profileRevision"] = json!(2);
        model["requiredCapabilities"] =
            r.json_property(workplan, "next-nc required capabilities")?;
    }
    let mut sections = Vec::new();
    let mut sources = Vec::new();
    for (i, (ws, link)) in
        r.g.sequence("MACHINING_PROCESS_SEQUENCE_RELATIONSHIP", workplan)?
            .into_iter()
            .enumerate()
    {
        let (section, mut source) = r.section(ws).map_err(|mut e| {
            if e.source.is_none() {
                e.source =
                    r.g.doc
                        .records
                        .get(&ws)
                        .map(|r| Box::new(r.location.clone()));
            }
            e.with("section", i + 1)
        })?;
        source["sequenceRelationship"] = r.location(link);
        sections.push(section);
        sources.push(source);
    }
    r.g.finish()?;
    model["sections"] = Json::Array(sections);
    let required = requirements(&model)?;
    if revision == 2 && model["requiredCapabilities"] != json!(required) {
        return Err(r.g.fail(
            workplan,
            "CAPABILITIES",
            "Declared capabilities differ from complete job requirements",
        ));
    }
    let schema = format!("next-nc/decoded-program/{revision}");
    let fingerprint = Fingerprint {
        schema: schema.clone(),
        algorithm: "sha256".into(),
        value: json::fingerprint(&schema, &model)?,
    };
    let mut provenance = json!({"schema":"next-nc/source-provenance/2","inputSHA256":doc.input_sha256,"columnConvention":"one-based UTF-8 byte column","workplan":r.location(workplan),"geometryContext":r.location(context)});
    // Move the owned provenance tree into its envelope. json! would serialize a
    // borrowed Vec and duplicate every operation/path/source record at peak RAM.
    provenance["sections"] = Json::Array(sources);
    let report=Report{profile,machine:if milling{"mill"}else{"lathe"}.into(),units:length,entities:doc.records.len(),sections:model["sections"].as_array().map_or(0,Vec::len),paths:r.paths,expanded_items:r.expanded,fingerprint,bounds:json!({"min":r.min,"max":r.max}),required_capabilities:required,validation:"Next-NC source profile only; not AP238 certification, machine/setup validation or execution authorization"};
    Ok(Program {
        model,
        report,
        provenance,
    })
}
pub fn decode(text: &str, limits: &Limits) -> Result<Program> {
    decode_document(&parse(text, limits)?, limits)
}
