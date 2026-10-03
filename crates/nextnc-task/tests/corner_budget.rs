use motion_command::{Feed, Machine};
use nextnc_native::{compiled, part21::Limits};
use nextnc_task::{
    binding::*,
    lowering::*,
    spindle::{Capability, FeedbackPolicy},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn snapshot(lathe: bool) -> Snapshot {
    Snapshot {
        machine: if lathe {
            Machine::LatheXz
        } else {
            Machine::MillXyz
        },
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
        shaping: Shaping::Disabled,
        reverse_spindle: true,
        maximum_rpm: 2000.,
        flood: true,
        mist: true,
        spindle: lathe.then_some(Capability {
            identity: [1; 32],
            css: true,
            feedback: FeedbackPolicy {
                maximum_rps: 40.,
                heartbeat_timeout_s: 0.1,
                comparison_window_s: 0.05,
                position_error_revs: 0.002,
                relative_error: 0.05,
            },
        }),
    }
}

#[test]
fn original_dense_sources_keep_every_move_and_feed_with_period_derived_limits() -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corner-budget-fixtures");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json"))?)?;
    let cases = manifest["cases"]
        .as_array()
        .ok_or("fixture cases missing")?;
    assert_eq!(cases.len(), 6);
    for case in cases {
        let name = case["name"].as_str().ok_or("fixture name missing")?;
        let mut texts = Vec::new();
        for (suffix, key) in [("stpnc", "sourceSHA256"), ("plan.json", "setupSHA256")] {
            let text = std::fs::read(root.join(format!("{name}.{suffix}")))?;
            assert_eq!(
                format!("{:x}", Sha256::digest(&text)),
                case[key].as_str().ok_or("digest missing")?
            );
            texts.push(String::from_utf8(text)?);
        }
        let prepared = compiled::prepare(&texts[0], &texts[1], &Limits::default())?;
        let bound = bind(&prepared, &snapshot(name.starts_with("lathe")))?;
        for ns in [500_000, 1_000_000, 2_000_000] {
            let d = Dynamics {
                axis_mask: if name.starts_with("lathe") { 5 } else { 7 },
                interpolation_period_ns: ns,
                axes: [AxisDynamics {
                    velocity_mm_s: 30.,
                    acceleration_mm_s2: 100.,
                    jerk_mm_s3: 1000.,
                }; 3],
            };
            let plan = lower(&bound, d)?;
            assert_eq!(plan.commands().len(), bound.records().len());
            let cuts: Vec<_> = plan
                .pieces()
                .iter()
                .filter_map(|p| match p.payload {
                    Payload::Motion {
                        motion, dynamics, ..
                    } if motion.feed != Feed::Rapid => Some((p, motion, dynamics)),
                    _ => None,
                })
                .collect();
            assert_eq!(cuts.len(), 512);
            for (piece, motion, limits) in &cuts {
                assert_eq!(
                    BoundAction::Motion(*motion),
                    bound.records()[piece.command].action
                );
                assert!(limits.velocity_mm_s <= limits.maximum_velocity_mm_s);
                assert!(limits.acceleration_mm_s2 > 0. && limits.jerk_mm_s3 > 0.);
            }
            if name.contains("collinear") {
                assert!(plan.corner_budgets().is_empty());
                for (_, _, limits) in cuts {
                    assert!((limits.maximum_velocity_mm_s - 30.).abs() < 1e-12);
                    assert!((limits.acceleration_mm_s2 - 100.).abs() < 1e-12);
                    assert!((limits.jerk_mm_s3 - 1000.).abs() < 1e-11);
                }
            } else {
                assert_eq!(plan.corner_budgets().len(), 1);
                let budget = &plan.corner_budgets()[0];
                assert_eq!(budget.commands.len(), 512);
                assert_eq!(budget.interpolation_period_ns, ns);
                let dt = f64::from(ns) * 1e-9;
                let v = budget.maximum_velocity_mm_s;
                let a = budget.acceleration_mm_s2;
                let j = budget.jerk_mm_s3;
                let jump = budget.maximum_direction_jump;
                assert!(a + jump * v / dt <= 100. + 1e-10);
                assert!(j + 0.75 * jump * a / dt + jump * v / dt.powi(2) <= 1000. + 1e-9);
                assert!(3. * dt * v <= budget.minimum_span_mm + 1e-14);
                if ns == 1_000_000 {
                    assert!((v - 0.462500020613395).abs() < 1e-8, "{name}: {v}");
                    assert_eq!((a, j), (50., 500.));
                }
                for (_, _, limits) in cuts {
                    assert_eq!(limits.maximum_velocity_mm_s, v);
                    assert_eq!(limits.acceleration_mm_s2, a);
                    assert_eq!(limits.jerk_mm_s3, j);
                }
            }
        }
    }
    Ok(())
}
