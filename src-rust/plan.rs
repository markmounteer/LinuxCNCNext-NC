//! Reviewed setup/transition data. Validation is never a physical-clearance claim.
use crate::{json, part21::Limits, profile::Program, Diagnostic, Result};
use serde::Serialize;
use serde_json::{json, Value};

const SCHEMAS: [&str; 5] = [
    "linuxcnc-next-nc/execution-plan/1",
    "linuxcnc-next-nc/execution-plan/2",
    "linuxcnc-next-nc/execution-plan/3",
    "linuxcnc-next-nc/execution-plan/4",
    "linuxcnc-next-nc/execution-plan/5",
];
pub const WORK_OFFSETS: [&str; 9] = [
    "G54", "G55", "G56", "G57", "G58", "G59", "G59.1", "G59.2", "G59.3",
];
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Waypoint {
    pub axis: usize,
    pub value: f64,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Mapping {
    pub tool: u32,
    pub offset: u32,
    pub work_offset: u8,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum Transition {
    Retract {
        retract: Vec<Waypoint>,
        approach: Vec<Waypoint>,
    },
    Continue,
    Link {
        moves: Vec<Waypoint>,
    },
}

/// Explicit additional execution policy, independent of source CAM tolerance.
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum PathControl {
    ExactPath,
    ExactStop,
    Blend { additional_deviation_mm: f64 },
}
impl PathControl {
    pub fn termination(self) -> motion_command::Termination {
        match self {
            Self::ExactPath => motion_command::Termination::ExactPath,
            Self::ExactStop => motion_command::Termination::ExactStop,
            Self::Blend {
                additional_deviation_mm,
            } => motion_command::Termination::Blend {
                max_deviation_mm: additional_deviation_mm,
            },
        }
    }
}

fn path_control(value: &Value, unit: f64) -> Result<PathControl> {
    keys(value, &["mode", "additionalDeviation"], "path control")?;
    match value["mode"].as_str() {
        Some("exactPath" | "exactStop") => {
            keys(value, &["mode"], "exact path control")?;
            Ok(if value["mode"] == "exactPath" {
                PathControl::ExactPath
            } else {
                PathControl::ExactStop
            })
        }
        Some("blend") => {
            let deviation = value["additionalDeviation"].as_f64().map(|v| v * unit)
                .filter(|v| v.is_finite() && *v > 0.0 && *v < 1e9)
                .ok_or_else(|| fail("PATH_CONTROL", "Blend requires an explicit positive additionalDeviation in plan units; CAM tolerance is not an execution allowance"))?;
            Ok(PathControl::Blend {
                additional_deviation_mm: deviation,
            })
        }
        _ => Err(fail(
            "PATH_CONTROL",
            "Path control must explicitly select exactPath, exactStop or blend",
        )),
    }
}
/// Only constructed after complete validation; callers receive read-only data.
#[derive(Clone, Debug, Serialize)]
pub struct ValidatedPlan {
    source: Value,
    mappings: Vec<Mapping>,
    transitions: Vec<Transition>,
    path_controls: Vec<PathControl>,
    end: Vec<Waypoint>,
}
impl ValidatedPlan {
    pub fn source(&self) -> &Value {
        &self.source
    }
    pub fn mappings(&self) -> &[Mapping] {
        &self.mappings
    }
    pub fn transitions(&self) -> &[Transition] {
        &self.transitions
    }
    pub fn end(&self) -> &[Waypoint] {
        &self.end
    }
    pub fn path_controls(&self) -> &[PathControl] {
        &self.path_controls
    }
}
fn fail(code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("execution-plan", code, message)
}
fn keys(value: &Value, allowed: &[&str], label: &str) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| fail("PLAN", format!("{label} must be an object")))?;
    if object.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(fail("PLAN", format!("Unknown fields in {label}")));
    }
    Ok(())
}
fn point(value: &Value) -> Result<[f64; 3]> {
    let a = value
        .as_array()
        .filter(|a| a.len() == 3)
        .ok_or_else(|| fail("MODEL", "Missing source point"))?;
    let mut p = [0.; 3];
    for (i, v) in a.iter().enumerate() {
        p[i] = v
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| fail("MODEL", "Invalid source coordinate"))?;
    }
    Ok(p)
}
pub fn exit_point(section: &Value) -> Result<[f64; 3]> {
    let mut position = point(&section["start"])?;
    for path in section["paths"]
        .as_array()
        .ok_or_else(|| fail("MODEL", "Missing source paths"))?
    {
        if path["kind"] == "dwell" || (path["kind"] == "arc" && path["fullCircle"] == true) {
            continue;
        }
        position = point(if path["end"].is_array() {
            &path["end"]
        } else {
            path["points"]
                .as_array()
                .and_then(|a| a.last())
                .ok_or_else(|| fail("MODEL", "Missing path endpoint"))?
        })?;
    }
    Ok(position)
}
fn compatible(previous: &Value, current: &Value, a: &Mapping, b: &Mapping) -> Result<()> {
    let last = previous["paths"].as_array().and_then(|a| a.last());
    let spindle = last.map_or(&previous["initialSpindle"], |p| &p["spindle"]);
    let coolant = last.map_or(&previous["initialCoolant"], |p| &p["coolant"]);
    let reason = if previous["tool"] != current["tool"] {
        Some("Fusion tool, offset or description differs")
    } else if previous["workOffset"] != current["workOffset"] {
        Some("Fusion work offset differs")
    } else if spindle != &current["initialSpindle"] {
        Some("Spindle mode, speed, direction or RPM cap differs")
    } else if coolant != &current["initialCoolant"] {
        Some("Coolant state differs")
    } else if a != b {
        Some("Mapped tool, offset or work coordinate system differs")
    } else {
        None
    };
    if let Some(reason) = reason {
        return Err(fail("LINK_STATE", reason));
    }
    Ok(())
}
fn waypoints(
    value: &Value,
    axes: &[usize],
    start: Option<[f64; 3]>,
) -> Result<(Vec<Waypoint>, [f64; 3])> {
    let values=value.as_array().filter(|a|a.len()>=if start.is_some(){1}else{axes.len()} && a.len()<=1000)
        .ok_or_else(||fail("TRANSITION","Explicit ordered single-axis waypoints required (at most 1000); missing moves are not inferred"))?;
    let mut end = start.unwrap_or([0.; 3]);
    let mut established = [start.is_some(); 3];
    let mut moves = Vec::new();
    for (i, v) in values.iter().enumerate() {
        let allowed: Vec<_> = axes.iter().map(|a| ["x", "y", "z"][*a]).collect();
        keys(v, &allowed, "waypoint").map_err(|e| e.with("waypoint", i + 1))?;
        let o = v.as_object().filter(|o| o.len() == 1).ok_or_else(|| {
            fail("TRANSITION", "Each waypoint must specify one axis").with("waypoint", i + 1)
        })?;
        let (key, value) = o
            .iter()
            .next()
            .ok_or_else(|| fail("TRANSITION", "Empty waypoint"))?;
        let axis = ["x", "y", "z"]
            .iter()
            .position(|k| *k == key)
            .filter(|a| axes.contains(a))
            .ok_or_else(|| {
                fail("TRANSITION", "Unsupported waypoint axis").with("waypoint", i + 1)
            })?;
        let coordinate = value
            .as_f64()
            .filter(|n| n.is_finite() && n.abs() < 1e9)
            .ok_or_else(|| {
                fail(
                    "TRANSITION",
                    "Waypoint must be finite with magnitude below 1e9",
                )
                .with("waypoint", i + 1)
            })?;
        end[axis] = coordinate;
        established[axis] = true;
        moves.push(Waypoint {
            axis,
            value: coordinate,
        });
    }
    if axes.iter().any(|i| !established[*i]) {
        return Err(fail(
            "TRANSITION",
            "Waypoints must establish every configured linear axis",
        ));
    }
    Ok((moves, end))
}
fn integer(v: &Value) -> Result<u32> {
    let n = v
        .as_f64()
        .filter(|n| n.is_finite() && n.fract() == 0. && *n > 0. && *n <= 99999.)
        .ok_or_else(|| {
            fail(
                "TOOL_MAPPING",
                "Tool and offset records must be explicit integers from 1 through 99999",
            )
        })?;
    Ok(n as u32)
}
fn section_context(error: Diagnostic, s: &Value, i: usize, field: impl Into<String>) -> Diagnostic {
    error
        .with("section", i + 1)
        .with("operation", s["name"].clone())
        .with("tool", s["tool"]["number"].clone())
        .with("field", field.into())
}
pub fn validate(value: &Value, program: &Program) -> Result<ValidatedPlan> {
    keys(
        value,
        &[
            "schema",
            "machine",
            "programFingerprint",
            "units",
            "tools",
            "workOffsets",
            "sections",
            "end",
            "pathControl",
        ],
        "execution plan",
    )?;
    let schema = value["schema"]
        .as_str()
        .and_then(|s| SCHEMAS.iter().position(|x| *x == s))
        .ok_or_else(|| fail("PLAN_SCHEMA", "Unsupported execution-plan schema"))?;
    let mill = program.model["machine"] == "mill";
    if if schema >= 3 {
        value["machine"] != if mill { "mill" } else { "lathe" }
    } else {
        mill || value.get("machine").is_some()
    } {
        return Err(fail(
            "PLAN_MACHINE",
            "Mill requires execution-plan/4 or /5; declared machine must match source",
        ));
    }
    if value["programFingerprint"] != program.report.fingerprint.value {
        return Err(fail(
            "PLAN_MISMATCH",
            "Plan belongs to a different decoded program; regenerate and review its moves",
        ));
    }
    if value["units"] != program.model["units"] {
        return Err(fail(
            "PLAN_UNITS",
            "Plan and source must use the same units",
        ));
    }
    let tools = value["tools"]
        .as_object()
        .ok_or_else(|| fail("TOOL_MAPPING", "Explicit tool mappings required"))?;
    let wcs = value["workOffsets"]
        .as_object()
        .ok_or_else(|| fail("WCS_MAPPING", "Explicit work-offset mappings required"))?;
    let sections = program.model["sections"]
        .as_array()
        .ok_or_else(|| fail("MODEL", "Missing source sections"))?;
    let transitions = value["sections"]
        .as_array()
        .filter(|a| a.len() == sections.len())
        .ok_or_else(|| fail("TRANSITION", "One transition is required per operation"))?;
    let axes: &[usize] = if mill { &[0, 1, 2] } else { &[0, 2] };
    let mut issues = Vec::new();
    let mut not_checked = Vec::new();
    let mut mappings = Vec::new();
    let path_controls = if schema == 4 {
        let controls = value["pathControl"]
            .as_array()
            .filter(|c| c.len() == sections.len())
            .ok_or_else(|| {
                fail(
                    "PATH_CONTROL",
                    "Execution-plan/5 requires one explicit pathControl per operation",
                )
            })?;
        let unit = if program.model["units"] == "inch" {
            25.4
        } else {
            1.0
        };
        controls
            .iter()
            .enumerate()
            .map(|(i, control)| match path_control(control, unit) {
                Ok(control) => Some(control),
                Err(error) => {
                    issues.push(section_context(
                        error,
                        &sections[i],
                        i,
                        format!("pathControl[{i}]"),
                    ));
                    None
                }
            })
            .collect::<Vec<_>>()
    } else {
        if value.get("pathControl").is_some() {
            return Err(fail(
                "PATH_CONTROL",
                "Explicit pathControl requires execution-plan/5",
            ));
        }
        vec![Some(PathControl::ExactPath); sections.len()]
    };
    for (i, s) in sections.iter().enumerate() {
        let key = format!("{}:{}", s["tool"]["number"], s["tool"]["offset"]);
        let mapped = (|| -> Result<(u32, u32)> {
            let t = tools
                .get(&key)
                .ok_or_else(|| fail("TOOL_MAPPING", format!("Missing Fusion tool:offset {key}")))?;
            keys(t, &["tool", "offset"], "tool mapping")?;
            Ok((integer(&t["tool"])?, integer(&t["offset"])?))
        })();
        let mapped = match mapped {
            Ok(v) => Some(v),
            Err(e) => {
                issues.push(section_context(e, s, i, format!("tools.{key}")));
                None
            }
        };
        let wcs_key = s["workOffset"].to_string();
        let work_offset = wcs
            .get(&wcs_key)
            .and_then(Value::as_str)
            .and_then(|s| WORK_OFFSETS.iter().position(|w| *w == s));
        if work_offset.is_none() {
            issues.push(section_context(
                fail(
                    "WCS_MAPPING",
                    format!("Map Fusion work offset {wcs_key} explicitly to G54..G59.3"),
                ),
                s,
                i,
                format!("workOffsets.{wcs_key}"),
            ));
        }
        mappings.push(
            mapped
                .zip(work_offset)
                .map(|((tool, offset), wcs)| Mapping {
                    tool,
                    offset,
                    work_offset: wcs as u8 + 1,
                }),
        );
    }
    let mut parsed = Vec::new();
    for (i, (s, t)) in sections.iter().zip(transitions).enumerate() {
        let result = (|| -> Result<Option<Transition>> {
            keys(
                t,
                if schema == 0 {
                    &["retract", "approach"]
                } else if schema == 1 {
                    &["mode", "retract", "approach"]
                } else {
                    &["mode", "retract", "approach", "moves"]
                },
                "section transition",
            )?;
            let mode = if schema == 0 {
                "retract"
            } else {
                t["mode"]
                    .as_str()
                    .ok_or_else(|| fail("TRANSITION", "Explicit transition mode required"))?
            };
            if !["continue", "retract", "link"].contains(&mode) || (schema == 1 && mode == "link") {
                return Err(fail("TRANSITION", "Unsupported transition mode"));
            }
            if schema != 0 {
                keys(
                    t,
                    match mode {
                        "continue" => &["mode"],
                        "link" => &["mode", "moves"],
                        _ => &["mode", "retract", "approach"],
                    },
                    "section transition",
                )?;
            }
            if mode == "retract" {
                // Collect both fields independently so a bad retract cannot hide
                // a second bad approach from the operator's preflight report.
                let retract = waypoints(&t["retract"], axes, None);
                let approach = waypoints(&t["approach"], axes, None).and_then(|(p, end)| {
                    if end != point(&s["start"])? {
                        return Err(fail(
                            "ENTRY_MISMATCH",
                            "Approach must end exactly at recorded operation entry",
                        )
                        .with("expected", s["start"].clone())
                        .with("actual", json!(end)));
                    }
                    Ok(p)
                });
                let retract = match retract {
                    Ok((p, _)) => Some(p),
                    Err(e) => {
                        issues.push(section_context(e, s, i, format!("sections[{i}].retract")));
                        None
                    }
                };
                let approach = match approach {
                    Ok(p) => Some(p),
                    Err(e) => {
                        issues.push(section_context(e, s, i, format!("sections[{i}].approach")));
                        None
                    }
                };
                return Ok(retract
                    .zip(approach)
                    .map(|(retract, approach)| Transition::Retract { retract, approach }));
            }
            if i == 0 {
                return Err(fail(
                    if mode == "continue" {
                        "CONTINUATION"
                    } else {
                        "LINK_STATE"
                    },
                    "First operation requires a reviewed machine approach",
                ));
            }
            let (Some(a), Some(b)) = (&mappings[i - 1], &mappings[i]) else {
                not_checked.push(json!({"section":i+1,"field":format!("sections[{i}]"),"reason":"Boundary checks require valid mappings for both operations"}));
                return Ok(None);
            };
            compatible(&sections[i - 1], s, a, b).map_err(|mut e| {
                if mode == "continue" {
                    e.code = "CONTINUATION".into();
                }
                e
            })?;
            let start = exit_point(&sections[i - 1])?;
            let entry = point(&s["start"])?;
            if mode == "continue" {
                if start != entry {
                    return Err(fail(
                        "CONTINUATION",
                        "Previous commanded exit and next entry differ; no move is inferred",
                    ));
                }
                return Ok(Some(Transition::Continue));
            }
            let (moves, end) = waypoints(&t["moves"], axes, Some(start))?;
            if end != entry {
                return Err(fail(
                    "LINK_ENDPOINT",
                    "Reviewed link must end exactly at next operation entry",
                )
                .with("expected", json!(entry))
                .with("actual", json!(end)));
            }
            Ok(Some(Transition::Link { moves }))
        })();
        match result {
            Ok(t) => parsed.push(t),
            Err(e) => {
                issues.push(section_context(e, s, i, format!("sections[{i}]")));
                parsed.push(None);
            }
        }
    }
    let end = match waypoints(&value["end"], axes, None) {
        Ok((p, _)) => Some(p),
        Err(e) => {
            issues.push(e.with("field", "end"));
            None
        }
    };
    if let Some(first) = issues.first() {
        return Err(first
            .clone()
            .with("issues", json!(issues))
            .with("notChecked", json!(not_checked)));
    }
    Ok(ValidatedPlan {
        source: value.clone(),
        mappings: mappings
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| fail("INTERNAL", "Incomplete validated mappings"))?,
        transitions: parsed
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| fail("INTERNAL", "Incomplete validated transitions"))?,
        end: end.ok_or_else(|| fail("INTERNAL", "Incomplete validated end"))?,
        path_controls: path_controls
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| fail("INTERNAL", "Incomplete validated path controls"))?,
    })
}
pub fn parse(text: &str, program: &Program, limits: &Limits) -> Result<ValidatedPlan> {
    validate(
        &json::parse(text, limits).map_err(|e| e.with("inputRole", "setup"))?,
        program,
    )
}
