//! Offline manifest compatibility. A declaration is never live negotiation.
use crate::{
    json,
    part21::Limits,
    profile::{self, Program},
    Diagnostic, Result,
};
use motion_command::{v2::GeometryCapabilities, Capabilities, Capability};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Manifest {
    schema: String,
    machine: String,
    motion_contract_version: u32,
    capabilities: Vec<String>,
    /// Evidence selected by the package owner, not fetched or authenticated here.
    evidence_sha256: String,
}
const BASE: [(&str, Capability); 12] = [
    ("linear", Capability::Linear),
    ("planar-arc", Capability::PlanarArc),
    ("blend", Capability::Blend),
    ("at-speed", Capability::AtSpeed),
    ("feed-per-revolution", Capability::FeedPerRevolution),
    ("spindle", Capability::Spindle),
    ("css", Capability::Css),
    ("coolant", Capability::Coolant),
    ("tool-change", Capability::ToolChange),
    ("tool-offset", Capability::ToolOffset),
    ("dwell", Capability::Dwell),
    ("completion", Capability::Completion),
];
fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("target-capabilities", "CAPABILITY_MANIFEST", message)
}
impl Manifest {
    pub fn parse(text: &str, limits: &Limits) -> Result<Self> {
        let value = json::parse(text, limits)?;
        let result: Self = serde_json::from_value(value).map_err(|e| fail(e.to_string()))?;
        if result.schema != "nextnc-native/target-capabilities/1"
            || !["mill", "lathe"].contains(&result.machine.as_str())
            || result.motion_contract_version != motion_command::v2::CONTRACT_VERSION
        {
            return Err(fail(
                "Unknown manifest schema, machine or shared motion contract revision",
            ));
        }
        if result.evidence_sha256.len() != 64
            || !result
                .evidence_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(fail("Evidence identity must be a lowercase SHA-256 digest"));
        }
        let mut seen = BTreeSet::new();
        for name in &result.capabilities {
            if !seen.insert(name.as_str())
                || !(BASE.iter().any(|(s, _)| s == name)
                    || ["helix", "multiple-turns"].contains(&name.as_str()))
            {
                return Err(fail(format!("Duplicate or unsupported capability {name}")));
            }
        }
        if (seen.contains("helix") || seen.contains("multiple-turns"))
            && !seen.contains("planar-arc")
        {
            return Err(fail(
                "Helix/multiple-turns also require planar-arc geometry support",
            ));
        }
        Ok(result)
    }
    pub fn declared_geometry(&self) -> GeometryCapabilities {
        let mut base = Capabilities::default();
        for (name, cap) in BASE {
            if self.capabilities.iter().any(|s| s == name) {
                base = base.with(cap);
            }
        }
        GeometryCapabilities {
            base,
            helix: self.capabilities.iter().any(|s| s == "helix"),
            multiple_turns: self.capabilities.iter().any(|s| s == "multiple-turns"),
        }
    }
}
pub fn check(text: Option<&str>, program: &Program, limits: &Limits) -> Result<Value> {
    check_required(
        text,
        program,
        limits,
        &profile::requirements(&program.model)?,
        "Offline source requirements against the supplied manifest only",
    )
}
/// Preparation adds reviewed waypoints and synchronization policy to source
/// requirements. A source-only capability pass must never qualify the result.
pub fn check_prepared(
    text: Option<&str>,
    prepared: &crate::compiled::PreparedPlan,
    limits: &Limits,
) -> Result<Value> {
    let mut required = profile::requirements(&prepared.program().model)?;
    required.extend(prepared.audit().required_capabilities.iter().cloned());
    required.sort();
    required.dedup();
    check_required(text, prepared.program(), limits, &required,
        "Offline complete prepared-plan requirements including compiler policy; no live negotiation")
}
fn check_required(
    text: Option<&str>,
    program: &Program,
    limits: &Limits,
    required: &[String],
    scope: &str,
) -> Result<Value> {
    let Some(text) = text else {
        return Ok(
            json!({"status":"not_checked","reason":"No target capability manifest supplied","liveNegotiation":"not_checked"}),
        );
    };
    let manifest = Manifest::parse(text, limits)?;
    if manifest.machine != program.report.machine {
        return Err(fail("Target machine differs from source profile"));
    }
    let unsupported: Vec<_> = required
        .iter()
        .filter(|name| !manifest.capabilities.contains(name))
        .collect();
    if !unsupported.is_empty() {
        return Err(Diagnostic::new("target-capabilities","UNSUPPORTED_CAPABILITIES","Complete job requires capabilities absent from the selected target manifest; no command stream was published").with("unsupported",json!(unsupported)).with("required",json!(required)));
    }
    Ok(
        json!({"status":"passed","scope":scope,"manifestSHA256":format!("{:x}",Sha256::digest(text.as_bytes())),"evidenceSHA256":manifest.evidence_sha256,"required":required,"liveNegotiation":"not_checked","executionAuthorized":false}),
    )
}
