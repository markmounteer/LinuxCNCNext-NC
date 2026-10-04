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
        shaping_kernel: None,
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
                trajectory: AxisDynamics {
                    velocity_mm_s: 10000.0,
                    acceleration_mm_s2: 10000.0,
                    jerk_mm_s3: 10000.0,
                },
                scalar_origin_mm: 0.0,
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
                    assert!(
                        limits.maximum_velocity_mm_s < 30. && limits.maximum_velocity_mm_s > 29.9
                    );
                    assert!(limits.acceleration_mm_s2 < 100. && limits.acceleration_mm_s2 > 99.9);
                    assert!(limits.jerk_mm_s3 < 1000. && limits.jerk_mm_s3 > 990.);
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
                    assert!(v < 0.462500020613395 && v > 0.46, "{name}: {v}");
                    assert!(a < 50. && a > 49.9);
                    assert!(j < 500. && j > 499.);
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

#[test]
fn original_split_join_counterexample_has_identical_limits_and_preserved_provenance() -> TestResult
{
    // Exact source/setup bytes from the independent configured-shim counterexample.
    // Only source polyline grouping differs; inspect every prepared/bound action
    // rather than feeding fabricated BoundRecords straight into the allocator.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corner-partition-fixtures");
    let mut results = Vec::new();
    let machine = Dynamics {
        axis_mask: 7,
        interpolation_period_ns: 1_000_000,
        trajectory: AxisDynamics {
            velocity_mm_s: 30.,
            acceleration_mm_s2: 100.,
            jerk_mm_s3: 1000.,
        },
        scalar_origin_mm: 0.,
        axes: [AxisDynamics {
            velocity_mm_s: 30.,
            acceleration_mm_s2: 100.,
            jerk_mm_s3: 1000.,
        }; 3],
    };
    for name in ["joined", "split"] {
        let source = std::fs::read_to_string(root.join(format!("{name}.stpnc")))?;
        let setup = std::fs::read_to_string(root.join(format!("{name}.plan.json")))?;
        let prepared = compiled::prepare(&source, &setup, &Limits::default())?;
        let bound = bind(&prepared, &snapshot(false))?;
        let lowered = lower(&bound, machine)?;
        let cuts: Vec<_> = bound
            .records()
            .iter()
            .filter(|r| matches!(r.action, BoundAction::Motion(m) if m.feed != Feed::Rapid))
            .collect();
        assert_eq!(cuts.len(), 3);
        assert_eq!(
            cuts.iter().map(|r| r.source.ordinal).collect::<Vec<_>>(),
            [Some(1), Some(2), Some(if name == "split" { 1 } else { 3 })]
        );
        let points = [[5., 4., 5.], [5.5, 4., 4.5], [6., 4., 4.], [7., 4.001, 3.]];
        for (i, record) in cuts.iter().enumerate() {
            let BoundAction::Motion(m) = record.action else {
                return Err("missing cut".into());
            };
            assert_eq!(&m.start_mm[..3], &points[i]);
            assert_eq!(&m.end_mm[..3], &points[i + 1]);
            assert_eq!(m.feed, Feed::PerSecond(2.));
        }
        assert_eq!(lowered.corner_budgets().len(), 1);
        assert_eq!(lowered.corner_budgets()[0].commands.len(), 3);
        assert!(lowered.corner_budgets()[0].maximum_velocity_mm_s < 0.67);
        results.push((bound, lowered));
    }
    let (joined, a) = &results[0];
    let (split, b) = &results[1];
    assert_eq!(
        joined
            .records()
            .iter()
            .map(|r| r.action)
            .collect::<Vec<_>>(),
        split.records().iter().map(|r| r.action).collect::<Vec<_>>()
    );
    assert_eq!(a.pieces(), b.pieces());
    assert_eq!(a.commands(), b.commands());
    assert_eq!(a.drains_before(), b.drains_before());
    assert_eq!(a.corner_budgets(), b.corner_budgets());
    Ok(())
}
