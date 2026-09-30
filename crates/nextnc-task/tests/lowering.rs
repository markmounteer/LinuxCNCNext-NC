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
        },
    )?)
}
fn dynamics(lathe: bool) -> Dynamics {
    Dynamics {
        axis_mask: if lathe { 5 } else { 7 },
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
    close(pieces[0].maximum_velocity_mm_s, 10.0 * 2f64.sqrt());
    close(pieces[0].acceleration_mm_s2, 100.0 * 2f64.sqrt());
    close(pieces[0].jerk_mm_s3, 1000.0 * 2f64.sqrt());
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
