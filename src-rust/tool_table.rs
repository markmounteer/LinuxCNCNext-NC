//! Syntax and mapped-record presence in a file snapshot, not a live tool binding.
use crate::{plan::ValidatedPlan, profile::Program, Diagnostic, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
fn fail(code: &str, message: impl Into<String>, line: usize) -> Diagnostic {
    Diagnostic::new("tool-table", code, message).with("toolTableLine", line)
}
fn decimal(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = usize::from(b.first().is_some_and(|b| *b == b'+' || *b == b'-'));
    let start = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let before = i - start;
    if b.get(i) == Some(&b'.') {
        i += 1;
    }
    let start = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if before + i - start == 0 {
        return false;
    }
    if b.get(i).is_some_and(|b| *b == b'e' || *b == b'E') {
        i += 1;
        if b.get(i).is_some_and(|b| *b == b'+' || *b == b'-') {
            i += 1;
        }
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == b.len()
}
pub fn check(text: Option<&str>, program: &Program, plan: &ValidatedPlan) -> Result<Value> {
    if plan.source()["programFingerprint"] != program.report.fingerprint.value {
        return Err(fail(
            "PLAN_MISMATCH",
            "Tool-table check received a plan bound to another source program",
            0,
        ));
    }
    let limitations=json!(["File snapshot only; not the table loaded by LinuxCNC or an external database.","Physical tool identity, geometry, offsets and pockets are not verified.","Work offsets, travel, clearance, spindle feedback and execution readiness are not verified."]);
    let Some(text) = text else {
        return Ok(
            json!({"status":"not_checked","reason":"No tool-table snapshot supplied","limitations":limitations}),
        );
    };
    if text.len() > 1024 * 1024 {
        return Err(fail("TOOL_TABLE", "Tool table exceeds 1 MiB", 0));
    }
    let mut records = BTreeMap::new();
    for (index, raw) in text.trim_start_matches('\u{feff}').split('\n').enumerate() {
        let line = raw.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = BTreeMap::new();
        for word in line.split_whitespace() {
            let first = word.as_bytes()[0].to_ascii_uppercase();
            if !b"TPXYZABCUVWDIJQ".contains(&first) {
                return Err(fail(
                    "TOOL_TABLE_SYNTAX",
                    format!("Unrecognized word: {word}"),
                    index + 1,
                ));
            }
            // The accepted ASCII prefix is one UTF-8 byte, so this boundary is valid.
            let number = &word[1..];
            if !decimal(number) {
                return Err(fail(
                    "TOOL_TABLE_SYNTAX",
                    format!("Invalid numeric word: {word}"),
                    index + 1,
                ));
            }
            let value = number
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .ok_or_else(|| {
                    fail("TOOL_TABLE_SYNTAX", "Nonfinite tool-table value", index + 1)
                })?;
            if fields.insert(first, value).is_some() {
                return Err(fail("TOOL_TABLE_SYNTAX", "Duplicate word", index + 1));
            }
            if b"TPQ".contains(&first) {
                let integer = number.strip_prefix('+').unwrap_or(number);
                if integer.is_empty()
                    || !integer.bytes().all(|b| b.is_ascii_digit())
                    || value > 9_007_199_254_740_991.
                    || (first == b'Q' && value > 9.)
                {
                    return Err(fail("TOOL_TABLE_SYNTAX","T/P require nonnegative safe integers; Q requires integer 0..9, without decimals/exponents",index+1));
                }
            }
        }
        let tool = fields
            .get(&b'T')
            .filter(|_| fields.contains_key(&b'P'))
            .ok_or_else(|| {
                fail(
                    "TOOL_TABLE_SYNTAX",
                    "Each record requires T and P",
                    index + 1,
                )
            })?;
        if records.insert(*tool as u64, index + 1).is_some() {
            return Err(fail(
                "TOOL_TABLE_DUPLICATE",
                format!("Duplicate tool T{tool}"),
                index + 1,
            ));
        }
    }
    let mut mappings = Vec::new();
    let mut missing = Vec::new();
    let sections = program.model["sections"]
        .as_array()
        .ok_or_else(|| fail("MODEL", "Missing source sections", 0))?;
    if sections.len() != plan.mappings().len() {
        return Err(fail(
            "PLAN_MISMATCH",
            "Source/plan section count differs",
            0,
        ));
    }
    for (i, (section, mapping)) in sections.iter().zip(plan.mappings()).enumerate() {
        let mut item = json!({"section":i+1,"operation":section["name"],"fusionTool":format!("{}:{}",section["tool"]["number"],section["tool"]["offset"]),"tool":mapping.tool,"offset":mapping.offset});
        let mut absent = Vec::new();
        for (role, n) in [
            ("selected tool", mapping.tool),
            ("H-offset", mapping.offset),
        ] {
            if !records.contains_key(&(n as u64)) {
                absent.push(json!({"role":role,"record":n}));
            }
        }
        if absent.is_empty() {
            item["toolTableLine"] = json!(records.get(&(mapping.tool as u64)));
            item["offsetTableLine"] = json!(records.get(&(mapping.offset as u64)));
            mappings.push(item);
        } else {
            item["missing"] = json!(absent);
            missing.push(item);
        }
    }
    if !missing.is_empty() {
        return Err(fail(
            "TOOL_TABLE_MISSING",
            format!(
                "{} operations reference absent tool or offset records",
                missing.len()
            ),
            0,
        )
        .with("operations", json!(missing)));
    }
    Ok(
        json!({"status":"passed","scope":"File syntax and mapped T/H record presence only","sha256":format!("{:x}",Sha256::digest(text.as_bytes())),"records":records.len(),"mappings":mappings,"limitations":limitations}),
    )
}
