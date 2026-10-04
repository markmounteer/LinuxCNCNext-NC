//! Resolve the emitted dimensional unit graph with cycle/depth bounds.
use crate::{
    part21::{Document, Value},
    Diagnostic, Result,
};
use std::collections::{BTreeMap, BTreeSet};

pub struct Units<'a> {
    doc: &'a Document,
    cache: BTreeMap<u64, String>,
    active: BTreeSet<u64>,
    depth_limit: usize,
}
impl<'a> Units<'a> {
    pub fn new(doc: &'a Document, depth_limit: usize) -> Self {
        Self {
            doc,
            cache: BTreeMap::new(),
            active: BTreeSet::new(),
            depth_limit: depth_limit.min(128),
        }
    }
    fn fail(&self, id: u64, message: &str) -> Diagnostic {
        let error = Diagnostic::new("units", "UNITS", message);
        match self.doc.records.get(&id) {
            Some(r) => error.at(r.location.clone()),
            None => error,
        }
    }
    fn reference(&self, value: &Value, id: u64) -> Result<u64> {
        value
            .reference()
            .ok_or_else(|| self.fail(id, "Expected unit reference"))
    }
    fn dimensions(&self, id: u64, expected: [f64; 7]) -> Result<()> {
        let e = self.doc.entity(id, "DIMENSIONAL_EXPONENTS")?;
        if e.args.len() != 7
            || e.args
                .iter()
                .zip(expected)
                .any(|(a, b)| a.number() != Some(b))
        {
            return Err(self.fail(id, "Wrong dimensional exponents"));
        }
        Ok(())
    }
    pub fn resolve(&mut self, id: u64) -> Result<String> {
        if let Some(unit) = self.cache.get(&id) {
            return Ok(unit.clone());
        }
        if self.active.len() >= self.depth_limit {
            return Err(self.fail(id, "Unit graph depth limit exceeded"));
        }
        if !self.active.insert(id) {
            return Err(self.fail(id, "Cyclic unit graph"));
        }
        let result = self.resolve_inner(id);
        self.active.remove(&id);
        let unit = result?;
        self.cache.insert(id, unit.clone());
        Ok(unit)
    }
    fn resolve_inner(&mut self, id: u64) -> Result<String> {
        let r = self
            .doc
            .records
            .get(&id)
            .ok_or_else(|| self.fail(id, "Missing unit"))?;
        let has = |name: &str| r.parts.iter().any(|e| e.name == name);
        if has("SI_UNIT") {
            let e = self.doc.entity(id, "SI_UNIT")?;
            if e.args.len() != 2 {
                return Err(self.fail(id, "Invalid SI unit arity"));
            }
            let prefix = e.args[0].symbol();
            let name = e.args[1].symbol();
            for (component, prefix_wanted, name_wanted, result) in [
                ("LENGTH_UNIT", ".MILLI.", ".METRE.", "mm"),
                ("TIME_UNIT", "$", ".SECOND.", "second"),
                ("PLANE_ANGLE_UNIT", "$", ".RADIAN.", "radian"),
                ("SOLID_ANGLE_UNIT", "$", ".STERADIAN.", "steradian"),
            ] {
                if has(component) && prefix == Some(prefix_wanted) && name == Some(name_wanted) {
                    return Ok(result.into());
                }
            }
        } else if has("CONVERSION_BASED_UNIT") {
            let e = self.doc.entity(id, "CONVERSION_BASED_UNIT")?;
            if e.args.len() != 2 {
                return Err(self.fail(id, "Invalid conversion unit arity"));
            }
            let name = e.args[0]
                .string()
                .ok_or_else(|| self.fail(id, "Invalid unit name"))?
                .to_owned();
            let length = name == "inch";
            let time = name == "minute";
            if !((length && has("LENGTH_UNIT")) || (time && has("TIME_UNIT"))) {
                return Err(self.fail(id, "Unsupported conversion unit"));
            }
            let factor = self.reference(&e.args[1], id)?;
            let e = self.doc.entity(
                factor,
                if length {
                    "LENGTH_MEASURE_WITH_UNIT"
                } else {
                    "TIME_MEASURE_WITH_UNIT"
                },
            )?;
            if e.args.len() != 2 {
                return Err(self.fail(factor, "Invalid conversion measure arity"));
            }
            let expected_type = if length {
                "LENGTH_MEASURE"
            } else {
                "TIME_MEASURE"
            };
            let expected_value = if length { 25.4 } else { 60.0 };
            let good = matches!(&e.args[0],Value::Typed(t) if t.name==expected_type && t.args.len()==1 && t.args[0].number()==Some(expected_value));
            if !good {
                return Err(self.fail(factor, "Incorrect conversion factor"));
            }
            let base = self.reference(&e.args[1], factor)?;
            if self.resolve(base)? != if length { "mm" } else { "second" } {
                return Err(self.fail(factor, "Incorrect base conversion unit"));
            }
            let named = self.doc.entity(id, "NAMED_UNIT")?;
            let dimension = named
                .args
                .first()
                .and_then(Value::reference)
                .ok_or_else(|| self.fail(id, "Missing dimensions"))?;
            self.dimensions(
                dimension,
                if length {
                    [1., 0., 0., 0., 0., 0., 0.]
                } else {
                    [0., 0., 1., 0., 0., 0., 0.]
                },
            )?;
            return Ok(name);
        } else if has("CONTEXT_DEPENDENT_UNIT") {
            let e = self.doc.entity(id, "CONTEXT_DEPENDENT_UNIT")?;
            if e.args.len() != 2 || e.args[1].string() != Some("revolution") {
                return Err(self.fail(id, "Unsupported context-dependent unit"));
            }
            self.dimensions(self.reference(&e.args[0], id)?, [0.; 7])?;
            return Ok("revolution".into());
        } else if has("DERIVED_UNIT") {
            let e = self.doc.entity(id, "DERIVED_UNIT")?;
            let elements = e
                .args
                .first()
                .and_then(Value::aggregate)
                .ok_or_else(|| self.fail(id, "Invalid derived unit"))?;
            if e.args.len() != 1 || elements.len() != 2 {
                return Err(self.fail(id, "Derived unit needs two elements"));
            }
            let mut numerator = None;
            let mut denominator = None;
            for value in elements {
                let element = self.reference(value, id)?;
                let e = self.doc.entity(element, "DERIVED_UNIT_ELEMENT")?;
                if e.args.len() != 2 {
                    return Err(self.fail(element, "Invalid unit element"));
                }
                let unit = self.reference(&e.args[0], element)?;
                match e.args[1].number() {
                    Some(1.0) if numerator.is_none() => numerator = Some(unit),
                    Some(-1.0) if denominator.is_none() => denominator = Some(unit),
                    _ => {
                        return Err(self.fail(
                            element,
                            "Derived unit must have one numerator and one denominator",
                        ))
                    }
                }
            }
            let a = numerator.ok_or_else(|| self.fail(id, "Missing numerator"))?;
            let b = denominator.ok_or_else(|| self.fail(id, "Missing denominator"))?;
            return Ok(format!("{}/{}", self.resolve(a)?, self.resolve(b)?));
        }
        Err(self.fail(id, "Unsupported unit"))
    }
    pub fn measure(&mut self, id: u64, expected: &str, value_type: &str) -> Result<f64> {
        let e = self.doc.entity(id, "MEASURE_REPRESENTATION_ITEM")?;
        if e.args.len() != 3 {
            return Err(self.fail(id, "Invalid measure arity"));
        }
        let value = match &e.args[1] {
            Value::Typed(t) if t.name == value_type && t.args.len() == 1 => t.args[0].number(),
            _ => None,
        }
        .filter(|v| v.is_finite())
        .ok_or_else(|| self.fail(id, "Invalid measure value/type"))?;
        let unit = self.reference(&e.args[2], id)?;
        if self.resolve(unit)? != expected {
            return Err(self.fail(id, "Measure dimension disagrees with expected units"));
        }
        Ok(value)
    }
    pub fn geometry_context(&mut self) -> Result<(u64, String)> {
        let entries: Vec<_> = self.doc.all("GLOBAL_UNIT_ASSIGNED_CONTEXT").collect();
        if entries.len() != 1 {
            return Err(Diagnostic::new(
                "units",
                "CONTEXT",
                "Expected one geometry unit context",
            ));
        }
        let (r, e) = entries[0];
        let id = r.id;
        let geo = self.doc.entity(id, "GEOMETRIC_REPRESENTATION_CONTEXT")?;
        if geo.args.len() != 1 || geo.args[0].number() != Some(3.0) {
            return Err(self.fail(id, "Expected 3D context"));
        }
        let values = e
            .args
            .first()
            .and_then(Value::aggregate)
            .ok_or_else(|| self.fail(id, "Invalid geometry units"))?;
        if values.len() != 3 {
            return Err(self.fail(id, "Expected length/radian/steradian units"));
        }
        let ids: Vec<_> = values
            .iter()
            .map(|v| self.reference(v, id))
            .collect::<Result<_>>()?;
        let mut names = BTreeSet::new();
        for unit in ids {
            names.insert(self.resolve(unit)?);
        }
        let units = if names.contains("mm") {
            "mm"
        } else if names.contains("inch") {
            "inch"
        } else {
            return Err(self.fail(id, "Unsupported length unit"));
        };
        if names.len() != 3 || !names.contains("radian") || !names.contains("steradian") {
            return Err(self.fail(id, "Invalid geometry units"));
        }
        Ok((id, units.into()))
    }
}
