use motion_command::Machine;
use nextnc_native::{compiled, part21::Limits};
use nextnc_task::{binding::*, lowering::*};
use std::collections::BTreeMap;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn fixture(name: &str) -> Result<BoundPlan, Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let p = compiled::prepare(
        &std::fs::read_to_string(root.join(format!("{name}.stpnc")))?,
        &std::fs::read_to_string(root.join(format!("{name}.plan.json")))?,
        &Limits::default(),
    )?;
    bind_prepared(p, name)
}
fn bind_prepared(
    p: compiled::PreparedPlan,
    name: &str,
) -> Result<BoundPlan, Box<dyn std::error::Error>> {
    Ok(bind(
        &p,
        &Snapshot {
            machine: if name.starts_with("lathe") {
                Machine::LatheXz
            } else {
                Machine::MillXyz
            },
            commanded_pose_mm: [0.0; 9],
            active_work_offset: 1,
            work_offsets: [WorkOffset {
                translation_mm: [0.0; 9],
                rotation_degrees: 0.0,
            }; 9],
            temporary_offset_mm: [0.0; 9],
            active_tool_offset_mm: [0.0; 9],
            tool_offsets_mm: BTreeMap::from([(1, [0.0; 9]), (2, [0.0; 9])]),
            limits: [AxisLimits {
                minimum_mm: -2000.0,
                maximum_mm: 2000.0,
            }; 3],
            shaping: Shaping::Disabled,
            reverse_spindle: false,
            maximum_rpm: 2000.0,
            flood: true,
            mist: true,
            spindle: None,
        },
    )?)
}

#[test]
fn reviewed_policy_changes_have_drain_barriers_without_per_vertex_modes() -> TestResult {
    use motion_command::Termination;
    for name in ["mill-mm", "lathe-mm", "mill-inch", "lathe-inch"] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests-rust/fixtures/path-control");
        let prepared = compiled::prepare(
            &std::fs::read_to_string(root.join(format!("{name}-path-control.stpnc")))?,
            &std::fs::read_to_string(root.join(format!("{name}-path-control.plan.json")))?,
            &Limits::default(),
        )?;
        let bound = bind_prepared(prepared, name)?;
        let plan = lower(&bound, dynamics(name.starts_with("lathe")))?;
        let changes: Vec<_> = plan
            .pieces()
            .iter()
            .filter_map(|p| {
                if let Payload::Termination(t) = p.payload {
                    Some((p.command, t))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(changes.len(), 7);
        for ((_, actual), expected) in changes.iter().zip([
            Termination::ExactPath,
            Termination::Blend {
                max_deviation_mm: 0.02,
            },
            Termination::Blend {
                max_deviation_mm: 0.2,
            },
            Termination::ExactPath,
            Termination::ExactStop,
            Termination::Blend {
                max_deviation_mm: 0.01,
            },
            Termination::ExactPath,
        ]) {
            match (actual, expected) {
                (
                    Termination::Blend {
                        max_deviation_mm: a,
                    },
                    Termination::Blend {
                        max_deviation_mm: b,
                    },
                ) => close(*a, b),
                _ => assert_eq!(*actual, expected),
            }
        }
        for (command, _) in changes.iter().skip(1) {
            assert!(
                plan.drains_before().contains(command),
                "changed policy lacks drain at {command}"
            );
        }
        assert_eq!(
            plan.pieces()
                .iter()
                .filter(|p| matches!(p.payload, Payload::Motion { .. }))
                .count(),
            bound
                .records()
                .iter()
                .filter(|r| matches!(r.action,BoundAction::Motion(m) if m.start_mm!=m.end_mm))
                .count()
        );
    }
    Ok(())
}
fn dynamics(lathe: bool) -> Dynamics {
    Dynamics {
        axis_mask: if lathe { 5 } else { 7 },
        interpolation_period_ns: 1_000_000,
        trajectory: AxisDynamics {
            velocity_mm_s: 10000.0,
            acceleration_mm_s2: 10000.0,
            jerk_mm_s3: 10000.0,
        },
        scalar_origin_mm: 0.0,
        axes: std::array::from_fn(|i| AxisDynamics {
            velocity_mm_s: [10.0, 12.0, 20.0][i],
            acceleration_mm_s2: [100.0, 200.0, 300.0][i],
            jerk_mm_s3: [1000.0, 2000.0, 3000.0][i],
        }),
    }
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-10 * a.abs().max(b.abs()).max(1.0),
        "{a} != {b}"
    );
}

#[test]
fn directional_line_dynamics_preserve_feed_and_controller_limits() -> TestResult {
    let bound = fixture("mill-mm-line")?;
    let p = lower(&bound, dynamics(false))?;
    let pieces: Vec<_> = p
        .pieces()
        .iter()
        .filter_map(|p| match p.payload {
            Payload::Motion {
                motion, dynamics, ..
            } if matches!(motion.feed, motion_command::Feed::PerSecond(_)) => Some(dynamics),
            _ => None,
        })
        .collect();
    assert_eq!(pieces.len(), 1);
    close(pieces[0].velocity_mm_s, 2.0);
    for (actual, physical) in [
        (pieces[0].maximum_velocity_mm_s, 10.0 * 2f64.sqrt()),
        (pieces[0].acceleration_mm_s2, 100.0 * 2f64.sqrt()),
        (pieces[0].jerk_mm_s3, 1000.0 * 2f64.sqrt()),
    ] {
        assert!(
            actual < physical && actual > 0.99 * physical,
            "reserve must preserve useful directional capacity: {actual}/{physical}"
        );
    }
    Ok(())
}

#[test]
fn numerical_budget_tracks_observed_frame_and_period_without_changing_geometry() -> TestResult {
    let bound = fixture("mill-mm-line")?;
    let mut d = dynamics(false);
    let first = lower(&bound, d)?;
    let b = first.numerical_budget();
    assert!(b.position_reserve_mm > 0.0);
    d.interpolation_period_ns *= 2;
    let slower = lower(&bound, d)?;
    close(
        b.jerk_reserve_mm_s3,
        8.0 * slower.numerical_budget().jerk_reserve_mm_s3,
    );
    close(
        b.acceleration_reserve_mm_s2,
        4.0 * slower.numerical_budget().acceleration_reserve_mm_s2,
    );
    close(
        b.velocity_reserve_mm_s,
        2.0 * slower.numerical_budget().velocity_reserve_mm_s,
    );
    d.scalar_origin_mm = 5000.0;
    let far = lower(&bound, d)?;
    assert!(far.numerical_budget().scalar_horizon_mm > 5000.0);
    assert!(far.numerical_budget().jerk_reserve_mm_s3 > b.jerk_reserve_mm_s3);
    let motions = |p: &Plan| {
        p.pieces()
            .iter()
            .filter_map(|p| match p.payload {
                Payload::Motion { motion, .. } => Some(motion),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(motions(&first), motions(&slower));
    assert_eq!(motions(&first), motions(&far));
    Ok(())
}

#[test]
fn trajectory_ceiling_keeps_its_reserve_when_below_directional_axis_limits() -> TestResult {
    let bound = fixture("mill-mm-line")?;
    let mut d = dynamics(false);
    d.trajectory = AxisDynamics {
        velocity_mm_s: 1.0,
        acceleration_mm_s2: 2.0,
        jerk_mm_s3: 3.0,
    };
    let plan = lower(&bound, d)?;
    let b = plan.numerical_budget();
    for piece in plan.pieces() {
        if let Payload::Motion { dynamics, .. } = piece.payload {
            assert!(dynamics.maximum_velocity_mm_s + b.velocity_reserve_mm_s <= 1.0);
            assert!(dynamics.acceleration_mm_s2 + b.acceleration_reserve_mm_s2 <= 2.0);
            assert!(dynamics.jerk_mm_s3 + b.jerk_reserve_mm_s3 <= 3.0);
        }
    }
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        d.trajectory.jerk_mm_s3 = invalid;
        assert!(lower(&bound, d).is_err());
    }
    d = dynamics(false);
    d.interpolation_period_ns = 1;
    assert!(
        lower(&bound, d).is_err(),
        "unrepresentable reserve must refuse, not disappear"
    );
    Ok(())
}

#[test]
fn full_circles_helices_and_clockwise_turns_keep_analytic_task_conventions() -> TestResult {
    for (name, turn) in [
        ("mill-mm-arc-xy", 0),
        ("mill-mm-arc-xz", 0),
        ("mill-mm-arc-yz", 0),
        ("mill-mm-full-xy", 0),
        ("mill-mm-cw-xy", -1),
        ("mill-mm-helix-xy", 0),
        ("mill-mm-multi-xy", 2),
        ("mill-inch-multi-xy", 2),
        ("lathe-mm-cw-xz", -1),
        ("lathe-inch-full-xz", 0),
    ] {
        let b = fixture(name)?;
        let p = lower(&b, dynamics(name.starts_with("lathe")))?;
        let circles: Vec<_> = p
            .pieces()
            .iter()
            .filter_map(|p| match p.payload {
                Payload::Motion {
                    motion,
                    dynamics,
                    turn: Some(t),
                } => Some((motion, dynamics, t)),
                _ => None,
            })
            .collect();
        assert_eq!(circles.len(), 1, "{name}");
        assert_eq!(circles[0].2, turn, "{name}");
        assert!(circles[0].1.maximum_velocity_mm_s > circles[0].1.velocity_mm_s);
        let source = b.records().iter().find_map(|r| match r.action {
            BoundAction::Motion(m) if m.circular.is_some() => Some(m),
            _ => None,
        });
        assert_eq!(Some(circles[0].0), source);
    }
    Ok(())
}

#[test]
fn state_deltas_do_not_add_a_termination_command_per_vertex() -> TestResult {
    let b = fixture("mill-mm-multi-xy")?;
    let p = lower(&b, dynamics(false))?;
    assert_eq!(p.commands().len(), b.records().len());
    assert_eq!(
        p.pieces()
            .iter()
            .filter(|p| matches!(p.payload, Payload::Termination(_)))
            .count(),
        1
    );
    assert!(p
        .pieces()
        .iter()
        .any(|p| matches!(p.payload, Payload::Stationary(_))));
    for (i, range) in p.commands().iter().enumerate() {
        assert!(!range.is_empty());
        for (ordinal, piece) in p.pieces()[range.clone()].iter().enumerate() {
            assert_eq!(piece.command, i);
            assert_eq!(piece.ordinal, ordinal);
        }
        if !matches!(b.records()[i].action, BoundAction::Motion(_)) {
            let expected = if i == 0 {
                3
            } else if b.records()[i].action == BoundAction::State(compiled::Action::ResetModes) {
                2
            } else {
                1
            };
            assert_eq!(range.len(), expected);
            assert_eq!(
                p.pieces()[range.end - 1].payload,
                Payload::State(b.records()[i].action)
            );
        }
    }
    Ok(())
}

#[test]
fn reset_modes_establishes_exact_path_before_any_native_motion() -> TestResult {
    use motion_command::Termination;
    let bound = fixture("mill-mm-line")?;
    let plan = lower(&bound, dynamics(false))?;
    assert_eq!(
        plan.pieces()[0].payload,
        Payload::Termination(Termination::ExactPath)
    );
    assert_eq!(plan.pieces()[0].command, 0);
    assert_eq!(
        plan.pieces()[2].payload,
        Payload::State(BoundAction::State(compiled::Action::ResetModes))
    );
    // The fixture stays in exact path, so neither its second reset nor any
    // vertex needs another controller termination command.
    assert_eq!(
        plan.pieces()
            .iter()
            .filter(|p| matches!(p.payload, Payload::Termination(_)))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn missing_nonfinite_or_incompatible_machine_dynamics_refuse_the_entire_plan() -> TestResult {
    let b = fixture("mill-mm-arc-yz")?;
    for kind in 0..5 {
        let mut d = dynamics(false);
        match kind {
            0 => d.axes[2].jerk_mm_s3 = 0.0,
            1 => d.axes[1].velocity_mm_s = f64::NAN,
            2 => d.axes[0].acceleration_mm_s2 = f64::INFINITY,
            3 => d.axis_mask = 3,
            _ => d.axis_mask = 5,
        }
        assert!(lower(&b, d).is_err());
    }
    Ok(())
}
