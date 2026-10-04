use motion_command::{Feed, Machine, Rotation, Termination};
use nextnc_native::{compiled, part21::Limits, profile};
use nextnc_task::{
    binding::*,
    lowering::*,
    shaping::{Kernel, Term},
    steps::Layout,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn snapshot() -> Result<Snapshot, Box<dyn std::error::Error>> {
    let k: Value = serde_json::from_str(include_str!("fixtures/common-xy-kernel.json"))?;
    let terms: Vec<(u32, f64)> = serde_json::from_value(k["terms"].clone())?;
    Ok(Snapshot {
        machine: Machine::MillXyz,
        commanded_pose_mm: [0.; 9],
        active_work_offset: 1,
        work_offsets: [WorkOffset {
            translation_mm: [0.; 9],
            rotation_degrees: 0.,
        }; 9],
        temporary_offset_mm: [0.; 9],
        active_tool_offset_mm: [0.; 9],
        tool_offsets_mm: BTreeMap::from([(1, [0.; 9]), (2, [0.; 9])]),
        limits: [AxisLimits {
            minimum_mm: -2000.,
            maximum_mm: 2000.,
        }; 3],
        shaping: Shaping::EngagedXy,
        shaping_kernel: Some(Kernel::from_terms(
            1_000_000,
            &terms
                .into_iter()
                .map(|(delay_ticks, weight)| Term {
                    delay_ticks,
                    weight,
                })
                .collect::<Vec<_>>(),
        )?),
        reverse_spindle: true,
        maximum_rpm: 2000.,
        flood: true,
        mist: true,
        spindle: None,
    })
}
fn dynamics() -> Dynamics {
    Dynamics {
        axis_mask: 7,
        interpolation_period_ns: 1_000_000,
        scalar_origin_mm: 0.,
        trajectory: AxisDynamics {
            velocity_mm_s: 30.,
            acceleration_mm_s2: 100.,
            jerk_mm_s3: 1000.,
        },
        axes: [AxisDynamics {
            velocity_mm_s: 30.,
            acceleration_mm_s2: 100.,
            jerk_mm_s3: 1000.,
        }; 3],
    }
}
fn inputs(folder: &str, name: &str) -> Result<(String, Value), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(folder);
    Ok((
        std::fs::read_to_string(root.join(format!("{name}.stpnc")))?,
        serde_json::from_slice(&std::fs::read(root.join(format!("{name}.plan.json")))?)?,
    ))
}
fn prepared(
    source: &str,
    mut setup: Value,
    allowance: Option<f64>,
) -> Result<compiled::PreparedPlan, Box<dyn std::error::Error>> {
    // Every changed test source has its actual semantic identity recomputed;
    // the fixture on disk and its existing reviewed policy remain unchanged.
    setup["programFingerprint"] = profile::decode(source, &Limits::default())?
        .report
        .fingerprint
        .value
        .into();
    setup["schema"] = "linuxcnc-next-nc/execution-plan/5".into();
    setup["pathControl"] = json!([match allowance {
        Some(p) => json!({"mode":"blend", "additionalDeviation":p}),
        None => json!({"mode":"exactPath"}),
    }]);
    Ok(compiled::prepare(
        source,
        &setup.to_string(),
        &Limits::default(),
    )?)
}
fn cuts(plan: &Plan) -> Vec<(usize, Motion, ScalarDynamics, Option<i32>)> {
    plan.pieces()
        .iter()
        .filter_map(|p| match p.payload {
            Payload::Motion {
                motion,
                dynamics,
                turn,
            } if motion.feed != Feed::Rapid => Some((p.command, motion, dynamics, turn)),
            _ => None,
        })
        .collect()
}

#[test]
fn exact_curve_refuses_and_cam_tolerance_is_never_reused() -> TestResult {
    let (source, setup) = inputs("fixtures", "mill-mm-arc-xy")?;
    for cam in [0.002, 10.] {
        let source = source.replace("\"value\":0.002", &format!("\"value\":{cam}"));
        let p = prepared(&source, setup.clone(), None)?;
        let bound = bind(&p, &snapshot()?)?;
        let e = lower(&bound, dynamics())
            .err()
            .ok_or("exact curve accepted")?;
        assert!(e.reason.contains("CAM tolerance cannot be spent again"));
        assert!(e.command.is_some());
    }
    Ok(())
}

#[test]
fn approved_budget_caps_the_payload_without_changing_source_geometry_or_feed() -> TestResult {
    let (source, setup) = inputs("fixtures", "mill-mm-arc-xy")?;
    let p = prepared(&source, setup.clone(), Some(0.002))?;
    let s = snapshot()?;
    let bound = bind(&p, &s)?;
    let plan = lower(&bound, dynamics())?;
    Layout::from_lowered(&p, &bound, &plan)?;
    let budgets = plan.shaping_budgets();
    assert_eq!(budgets.len(), 1);
    let c = budgets[0].certificate;
    assert_eq!(c.allowance.source_corridor_mm, 0.002);
    assert!(c.total_error_upper_mm <= 0.002);
    assert!(c.allowance.arithmetic_error_mm > 0.);
    assert_eq!(
        c.kernel_identity,
        s.shaping_kernel.as_ref().ok_or("kernel")?.identity()
    );
    assert!(c.maximum_velocity_mm_s > 1. && c.maximum_velocity_mm_s < 3.);
    for (command, actual, limits, turn) in cuts(&plan) {
        let BoundAction::Motion(original) = bound.records()[command].action else {
            return Err("motion".into());
        };
        assert_eq!(
            actual,
            Motion {
                termination: Termination::ExactPath,
                ..original
            }
        );
        assert_eq!(turn, Some(0));
        assert!(limits.maximum_velocity_mm_s <= c.maximum_velocity_mm_s);
        assert!(limits.velocity_mm_s <= limits.maximum_velocity_mm_s);
    }
    // A certificate must not be reused with a different source allowance or kernel.
    let other = prepared(&source, setup, Some(0.001))?;
    assert!(Layout::from_lowered(&other, &bind(&other, &s)?, &plan).is_err());
    let mut changed = s;
    changed.shaping_kernel = Some(Kernel::from_terms(
        1_000_000,
        &[Term {
            delay_ticks: 0,
            weight: 1.,
        }],
    )?);
    assert!(Layout::from_lowered(&p, &bind(&p, &changed)?, &plan).is_err());
    Ok(())
}

// Reuse real AP238 path declarations, adding two analytic uses with independent
// feeds. First reverse on the same circle, then transfer to another circle.
fn three_arcs(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    let block = source
        .split("#93=")
        .nth(1)
        .ok_or("path")?
        .split("#126=")
        .next()
        .ok_or("end path")?;
    let block = format!("#93={block}");
    let old = "{\"start\":[5,4,5],\"end\":[3,6,5],\"center\":[3,4,5],\"plane\":\"XY\",\"clockwise\":false,\"sweepRadians\":1.5707963267948966,\"axialRise\":0}";
    let mut additions = String::new();
    for (i, arc, feed) in [
        (
            1,
            json!({"start":[3,6,5],"end":[5,4,5],"center":[3,4,5],"plane":"XY","clockwise":true,"sweepRadians":std::f64::consts::FRAC_PI_2,"axialRise":0}),
            300,
        ),
        (
            2,
            json!({"start":[5,4,5],"end":[7,6,5],"center":[7,4,5],"plane":"XY","clockwise":true,"sweepRadians":std::f64::consts::FRAC_PI_2,"axialRise":0}),
            60,
        ),
    ] {
        let mut new = block.replace(old, &arc.to_string());
        // Descending substitution avoids touching newly assigned identifiers.
        for id in (93..=125).rev() {
            for suffix in ['=', ',', ')'] {
                new = new.replace(
                    &format!("#{id}{suffix}"),
                    &format!("#{}{suffix}", id + i * 100),
                );
            }
        }
        new = new.replace(
            &format!("#48,#{},1.)", 93 + i * 100),
            &format!("#48,#{},{}.)", 93 + i * 100, i + 1),
        );
        new = new.replace(
            "NUMERIC_MEASURE(120.)",
            &format!("NUMERIC_MEASURE({feed}.)"),
        );
        additions.push_str(&new);
    }
    Ok(source
        .replace("#48,#126,2.)", "#48,#126,4.)")
        .replace("ENDSEC;\nEND-ISO", &format!("{additions}ENDSEC;\nEND-ISO")))
}

#[test]
fn same_circle_reversal_and_feed_change_share_history_but_circle_transfer_drains() -> TestResult {
    let (source, setup) = inputs("fixtures", "mill-mm-arc-xy")?;
    let source = three_arcs(&source.replace("\r\n", "\n"))?;
    let p = prepared(&source, setup, Some(0.002))?;
    let bound = bind(&p, &snapshot()?)?;
    let plan = lower(&bound, dynamics())?;
    Layout::from_lowered(&p, &bound, &plan)?;
    let moves = cuts(&plan);
    assert_eq!(moves.len(), 3);
    assert_eq!(plan.shaping_budgets().len(), 2);
    assert_eq!(
        moves[0].1.circular.ok_or("arc")?.rotation,
        Rotation::Counterclockwise
    );
    assert_eq!(
        moves[1].1.circular.ok_or("arc")?.rotation,
        Rotation::Clockwise
    );
    assert_eq!(moves[1].3, Some(-1));
    assert_eq!(
        moves.iter().map(|m| m.1.feed).collect::<Vec<_>>(),
        vec![
            Feed::PerSecond(2.),
            Feed::PerSecond(5.),
            Feed::PerSecond(1.)
        ]
    );
    assert!(!plan.drains_before().contains(&moves[1].0));
    assert!(plan.drains_before().contains(&moves[2].0));
    assert_eq!(
        moves[0].2.maximum_velocity_mm_s,
        moves[1].2.maximum_velocity_mm_s
    );
    Ok(())
}

#[test]
fn polyline_budget_uses_retained_local_corners_and_preserves_every_vertex() -> TestResult {
    let (source, setup) = inputs("corner-budget-fixtures", "mill-faceted-g94")?;
    let p = prepared(&source, setup, Some(0.002))?;
    let bound = bind(&p, &snapshot()?)?;
    let plan = lower(&bound, dynamics())?;
    Layout::from_lowered(&p, &bound, &plan)?;
    assert_eq!(cuts(&plan).len(), 512);
    assert_eq!(plan.shaping_budgets().len(), 1);
    let b = &plan.shaping_budgets()[0];
    assert!(b.certificate.geometry.tangent_jump_sum > 0.);
    let directions: Vec<_> = cuts(&plan)
        .into_iter()
        .map(|(_, m, _, _)| {
            let dx = m.end_mm[0] - m.start_mm[0];
            let dy = m.end_mm[1] - m.start_mm[1];
            let n = dx.hypot(dy);
            [dx / n, dy / n]
        })
        .collect();
    let all_corners: f64 = directions
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .sum();
    assert!(
        b.certificate.geometry.tangent_jump_sum < all_corners / 4.,
        "{b:?}; all corners {all_corners}"
    );
    assert!(b.certificate.total_error_upper_mm <= 0.002);
    for (command, actual, limits, _) in cuts(&plan) {
        let BoundAction::Motion(original) = bound.records()[command].action else {
            return Err("motion".into());
        };
        assert_eq!(
            actual,
            Motion {
                termination: Termination::ExactPath,
                ..original
            }
        );
        assert!(limits.maximum_velocity_mm_s <= b.certificate.maximum_velocity_mm_s);
    }
    Ok(())
}

#[test]
fn exact_axis_straights_do_not_acquire_an_invented_corner_budget() -> TestResult {
    let (source, setup) = inputs("corner-budget-fixtures", "mill-collinear-g94")?;
    let p = prepared(&source, setup, None)?;
    let plan = lower(&bind(&p, &snapshot()?)?, dynamics())?;
    assert_eq!(cuts(&plan).len(), 512);
    assert!(plan.shaping_budgets().is_empty());
    Ok(())
}

#[test]
fn period_mismatch_and_exhausted_reserve_refuse_instead_of_lifting_the_budget() -> TestResult {
    let (source, setup) = inputs("fixtures", "mill-mm-arc-xy")?;
    let p = prepared(&source, setup.clone(), Some(0.002))?;
    let bound = bind(&p, &snapshot()?)?;
    let mut d = dynamics();
    d.interpolation_period_ns = 2_000_000;
    assert!(lower(&bound, d)
        .err()
        .ok_or("period accepted")?
        .reason
        .contains("period differs"));
    let p = prepared(&source, setup, Some(1e-20))?;
    assert!(lower(&bind(&p, &snapshot()?)?, dynamics()).is_err());
    Ok(())
}

#[test]
fn disabled_shaping_preserves_reviewed_blending_and_analytic_motion() -> TestResult {
    let (source, setup) = inputs("fixtures", "mill-mm-arc-xy")?;
    let p = prepared(&source, setup, Some(0.002))?;
    let mut s = snapshot()?;
    s.shaping = Shaping::Disabled;
    s.shaping_kernel = None;
    let bound = bind(&p, &s)?;
    let plan = lower(&bound, dynamics())?;
    Layout::from_lowered(&p, &bound, &plan)?;
    assert!(plan.shaping_budgets().is_empty());
    assert_eq!(
        cuts(&plan)[0].1.termination,
        Termination::Blend {
            max_deviation_mm: 0.002
        }
    );
    assert!(cuts(&plan)[0].2.maximum_velocity_mm_s > 3.);
    Ok(())
}
