//! Explicit accounting for the exact-path baseline, not permission to approximate.
//! Source CAM tolerance and observed numeric residuals are not execution budgets.
use crate::{
    compiled::{self, Action, Phase, PreparedPlan},
    contract::{v2::Tolerance, Termination},
    geometry, Diagnostic, Result,
};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Operation {
    pub section: usize,
    pub cam_tolerance_mm: Option<f64>,
    pub cam_tolerance_provenance: &'static str,
    pub cam_design_error_bound_verified: bool,
    pub post_error_bound_mm: Option<f64>,
    pub fit_allocated_mm: f64,
    pub fit_used_mm: f64,
    pub blend_allocated_mm: f64,
    pub blend_used_mm: f64,
    pub numeric_error_bound_mm: Option<f64>,
    pub maximum_observed_arc_endpoint_residual_mm: f64,
    pub pre_shaper_total_bound_mm: Option<f64>,
    pub source_motions: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub policy: &'static str,
    pub coordinate_units: &'static str,
    pub approximation_enabled: bool,
    pub execution_authorized: bool,
    pub operations: Vec<Operation>,
    pub unqualified_terms: [&'static str; 4],
    pub residual_note: &'static str,
}

/// Inspect every prepared motion. A future positive-blend/fitting policy must
/// supply its own approved allocations and independently established bounds;
/// it cannot quietly inherit zero accounting from this baseline reporter.
pub fn describe(prepared: &PreparedPlan) -> Result<Report> {
    let unit = compiled::scale(prepared.program());
    let sections = compiled::array(&prepared.program().model["sections"])?;
    let mut operations = Vec::with_capacity(sections.len());
    for (index, section) in sections.iter().enumerate() {
        let (value, provenance) = match compiled::tolerance(&section["tolerance"], unit)? {
            Tolerance::Missing => (None, "missing"),
            Tolerance::SourceDeclaredMm(v) => (Some(v), "source-declared"),
            Tolerance::FusionOperationMm(v) => (Some(v), "fusion:operation:tolerance"),
        };
        operations.push(Operation {
            section: index + 1,
            cam_tolerance_mm: value,
            cam_tolerance_provenance: provenance,
            cam_design_error_bound_verified: false,
            post_error_bound_mm: None,
            fit_allocated_mm: 0.0,
            fit_used_mm: 0.0,
            blend_allocated_mm: 0.0,
            blend_used_mm: 0.0,
            numeric_error_bound_mm: None,
            maximum_observed_arc_endpoint_residual_mm: 0.0,
            pre_shaper_total_bound_mm: None,
            source_motions: 0,
        });
    }
    for span in prepared.spans() {
        for (offset, record) in prepared.commands()[span.commands.clone()]
            .iter()
            .enumerate()
        {
            if let Action::Motion(motion) = record.action {
                let Phase::Path { section, .. } = span.phase else {
                    return Err(Diagnostic::new(
                        "error-budget",
                        "UNACCOUNTED_MOTION",
                        "Motion has no source-operation budget",
                    ));
                };
                if motion.termination != Termination::ExactPath {
                    return Err(Diagnostic::new("error-budget", "UNACCOUNTED_APPROXIMATION", "This reporter qualifies only the exact-path baseline; a different policy needs explicit accounting")
                        .with("command", span.commands.start + offset).with("section", section + 1));
                }
                let metric = geometry::validate_with_floor(motion.geometry, 1e-7 * unit)?;
                let operation = operations.get_mut(section).ok_or_else(|| {
                    Diagnostic::new(
                        "error-budget",
                        "OPERATION",
                        "Motion refers to an unknown operation",
                    )
                })?;
                operation.source_motions += 1;
                operation.maximum_observed_arc_endpoint_residual_mm = operation
                    .maximum_observed_arc_endpoint_residual_mm
                    .max(metric.endpoint_discrepancy_mm);
            }
        }
    }
    Ok(Report {
        schema: "nextnc-native/error-budget/1",
        policy: compiled::POLICY,
        coordinate_units: "mm in source work frame; lathe X is radius",
        approximation_enabled: false,
        execution_authorized: false,
        operations,
        unqualified_terms: [
            "CAM tolerance is source metadata; no comparison to the original CAD design was performed",
            "Post approximation/error provenance is not supplied by the supported source profiles; zero is not inferred",
            "No independently established continuous numeric-error bound is available for the complete downstream path",
            "Shaping, kinematics, tracking, tooling and mechanical error require separate qualification",
        ],
        residual_note: "Endpoint residual is an observed numeric consistency metric, not a continuous path-error bound or spare fit/blend allowance. Missing bounds remain null; CAM tolerance is never spent again.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{json, part21::Limits};
    use serde_json::Value;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn baseline_never_spends_cam_tolerance_or_invents_missing_error_bounds() -> TestResult {
        for name in [
            "mill-mm-arcs-8",
            "mill-inch-arcs-8",
            "lathe-mm-arcs-8",
            "lathe-inch-arcs-8",
        ] {
            let root = format!("tests-rust/fixtures/benchmark/{name}");
            let p = compiled::prepare(
                &std::fs::read_to_string(format!("{root}.stpnc"))?,
                &std::fs::read_to_string(format!("{root}.plan.json"))?,
                &Limits::default(),
            )?;
            let r = describe(&p)?;
            let o = &r.operations[0];
            assert_eq!(
                o.cam_tolerance_mm,
                Some(if name.contains("inch") {
                    0.0001 * 25.4
                } else {
                    0.002
                })
            );
            assert_eq!(o.cam_tolerance_provenance, "source-declared");
            assert_eq!(o.source_motions, 8);
            assert_eq!(
                (
                    o.fit_allocated_mm,
                    o.fit_used_mm,
                    o.blend_allocated_mm,
                    o.blend_used_mm
                ),
                (0.0, 0.0, 0.0, 0.0)
            );
            assert!(
                !r.approximation_enabled
                    && !r.execution_authorized
                    && !o.cam_design_error_bound_verified
            );
            assert!(
                o.post_error_bound_mm.is_none()
                    && o.numeric_error_bound_mm.is_none()
                    && o.pre_shaper_total_bound_mm.is_none()
            );
            assert!(o.maximum_observed_arc_endpoint_residual_mm.is_finite());
            let report: Value = serde_json::to_value(r)?;
            assert_eq!(
                report["operations"][0]["pre_shaper_total_bound_mm"],
                Value::Null
            );
        }
        let fixture = json::parse(
            &std::fs::read_to_string("tests-rust/fixtures/legacy/mill-mm.json")?,
            &Limits::default(),
        )?;
        let p = compiled::prepare(
            fixture["text"].as_str().ok_or("source")?,
            &fixture["plan"].to_string(),
            &Limits::default(),
        )?;
        let r = describe(&p)?;
        assert_eq!(r.operations.len(), 4);
        assert!(r
            .operations
            .iter()
            .all(|o| o.cam_tolerance_mm.is_none() && o.cam_tolerance_provenance == "missing"));
        assert_eq!(
            r.operations.iter().map(|o| o.source_motions).sum::<usize>(),
            p.audit().motions
        );
        Ok(())
    }

    #[test]
    fn future_positive_blend_cannot_reuse_zero_budget_accounting() -> TestResult {
        let root = "tests-rust/fixtures/benchmark/mill-mm-arcs-8";
        let mut p = compiled::prepare(
            &std::fs::read_to_string(format!("{root}.stpnc"))?,
            &std::fs::read_to_string(format!("{root}.plan.json"))?,
            &Limits::default(),
        )?;
        let r = p
            .commands
            .iter_mut()
            .find(|r| matches!(r.action, Action::Motion(_)))
            .ok_or("motion")?;
        if let Action::Motion(ref mut m) = r.action {
            m.termination = Termination::Blend {
                max_deviation_mm: 0.001,
            };
        }
        assert_eq!(
            describe(&p).err().ok_or("unaccounted blend accepted")?.code,
            "UNACCOUNTED_APPROXIMATION"
        );
        Ok(())
    }
}
