//! The accepted profile entity/attribute grammar, ported as data from the pinned
//! legacy reader. Generation is a development step; runtime uses embedded data.
use crate::part21::{Document, Value};
use crate::{Diagnostic, Result};
use serde::Serialize;
use serde_json::Value as Json;
use std::collections::BTreeSet;
#[derive(Debug, Serialize)]
pub struct ShapeReport {
    pub records: usize,
    pub components: usize,
    pub attributes: usize,
}
fn matches(value: &Value, shape: &Json, doc: &Document) -> bool {
    match shape["kind"].as_str() {
        Some("string") => value.string().is_some(),
        Some("number") => value.number().is_some_and(f64::is_finite),
        Some("integer") => value.number().is_some_and(|v| {
            v.is_finite() && v.fract() == 0.0 && v.abs() <= 9_007_199_254_740_991.0
        }),
        Some("symbol") => shape["values"]
            .as_array()
            .is_some_and(|a| a.iter().any(|s| s.as_str() == value.symbol())),
        Some("reference") => value
            .reference()
            .and_then(|id| doc.records.get(&id))
            .is_some_and(|r| {
                shape["types"].as_array().is_some_and(|a| {
                    a.iter()
                        .any(|t| r.parts.iter().any(|p| Some(p.name.as_str()) == t.as_str()))
                })
            }),
        Some("choice") => shape["choices"]
            .as_array()
            .is_some_and(|a| a.iter().any(|s| matches(value, s, doc))),
        Some("aggregate") => value.aggregate().is_some_and(|a| {
            a.len() >= shape["min"].as_u64().unwrap_or(0) as usize
                && shape["max"].as_u64().is_none_or(|n| a.len() <= n as usize)
                && a.iter().all(|v| matches(v, &shape["item"], doc))
        }),
        Some("typed") => {
            if let Value::Typed(e) = value {
                e.args.len() == 1
                    && e.args[0].number().is_some_and(f64::is_finite)
                    && shape["types"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|s| s.as_str() == Some(&e.name)))
            } else {
                false
            }
        }
        Some("dimensions") => {
            value.symbol() == Some("*")
                || value
                    .reference()
                    .is_some_and(|id| doc.entity(id, "DIMENSIONAL_EXPONENTS").is_ok())
        }
        _ => false,
    }
}
pub fn validate(doc: &Document) -> Result<ShapeReport> {
    let grammar: Json = serde_json::from_str(include_str!("profile-shape.json"))
        .map_err(|e| Diagnostic::new("profile-shape", "INTERNAL_GRAMMAR", e.to_string()))?;
    if doc.schema != "INTEGRATED_CNC_SCHEMA" {
        return Err(Diagnostic::new(
            "profile-shape",
            "SCHEMA",
            "Unsupported schema",
        ));
    }
    let combos: Vec<BTreeSet<String>> =
        serde_json::from_value(grammar["complexCombinations"].clone())
            .map_err(|e| Diagnostic::new("profile-shape", "INTERNAL_GRAMMAR", e.to_string()))?;
    let complex_only: BTreeSet<_> = combos
        .iter()
        .flatten()
        .filter(|s| s.as_str() != "REPRESENTATION_CONTEXT")
        .collect();
    let mut report = ShapeReport {
        records: doc.records.len(),
        components: 0,
        attributes: 0,
    };
    for r in doc.records.values() {
        let names: BTreeSet<_> = r.parts.iter().map(|e| e.name.clone()).collect();
        if if r.parts.len() > 1 {
            !combos.contains(&names)
        } else {
            complex_only.contains(&r.parts[0].name)
        } {
            return Err(Diagnostic::new(
                "profile-shape",
                "COMPONENTS",
                "Unsupported entity component combination",
            )
            .at(r.location.clone()));
        }
        for e in &r.parts {
            let attributes = grammar["contract"][&e.name].as_array().ok_or_else(|| {
                Diagnostic::new(
                    "profile-shape",
                    "UNSUPPORTED_ENTITY",
                    format!("Unsupported {}", e.name),
                )
                .at(r.location.clone())
            })?;
            if e.args.len() != attributes.len() {
                return Err(Diagnostic::new(
                    "profile-shape",
                    "ARITY",
                    format!(
                        "{} needs {} attributes; found {}",
                        e.name,
                        attributes.len(),
                        e.args.len()
                    ),
                )
                .at(r.location.clone()));
            }
            for (i, (value, spec)) in e.args.iter().zip(attributes).enumerate() {
                let dimensions;
                let shape = if spec["shape"]["kind"] == "dimensions" {
                    dimensions = if names.contains("SI_UNIT") {
                        serde_json::json!({"kind":"symbol","values":["*"]})
                    } else {
                        serde_json::json!({"kind":"reference","types":["DIMENSIONAL_EXPONENTS"]})
                    };
                    &dimensions
                } else {
                    &spec["shape"]
                };
                if !matches(value, shape, doc) {
                    return Err(Diagnostic::new(
                        "profile-shape",
                        "ATTRIBUTE",
                        format!(
                            "{} parameter {} ({}) has invalid type/value",
                            e.name,
                            i + 1,
                            spec["attribute"]
                        ),
                    )
                    .at(r.location.clone()));
                }
                report.attributes += 1;
            }
            report.components += 1;
        }
    }
    Ok(report)
}
