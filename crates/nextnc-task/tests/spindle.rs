use motion_command::{Feed, Machine};
use nextnc_native::{
    compiled::{self, Action},
    part21::Limits,
};
use nextnc_task::{
    binding::*,
    lowering::*,
    spindle::{Capability, FeedbackPolicy},
};
use std::{collections::BTreeMap, path::Path};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn snapshot() -> Snapshot {
    let mut offsets = [WorkOffset {
        translation_mm: [0.0; 9],
        rotation_degrees: 0.0,
    }; 9];
    offsets[0].translation_mm[0] = 11.0;
    offsets[1].translation_mm[0] = 17.0;
    Snapshot {
        machine: Machine::LatheXz,
        commanded_pose_mm: [0.0; 9],
        active_work_offset: 1,
        work_offsets: offsets,
        temporary_offset_mm: [4.0, 0.0, -7.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        active_tool_offset_mm: [9.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        tool_offsets_mm: BTreeMap::from([
            (1, [0.0; 9]),
            (2, [0.7, 0.0, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            (3, [1.2, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        ]),
        limits: [AxisLimits {
            minimum_mm: -2000.0,
            maximum_mm: 2000.0,
        }; 3],
        shaping: Shaping::Disabled,
        shaping_kernel: None,
        reverse_spindle: true,
        maximum_rpm: 2000.0,
        flood: false,
        mist: false,
        spindle: Some(Capability {
            identity: [1; 32],
            css: true,
            feedback: FeedbackPolicy {
                maximum_rps: 40.0,
                heartbeat_timeout_s: 0.1,
                comparison_window_s: 0.05,
                position_error_revs: 0.002,
                relative_error: 0.05,
            },
        }),
    }
}
fn fixture(name: &str) -> Result<compiled::PreparedPlan, Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/spindle-fixtures");
    Ok(compiled::prepare(
        &std::fs::read_to_string(dir.join(format!("{name}.stpnc")))?,
        &std::fs::read_to_string(dir.join(format!("{name}.plan.json")))?,
        &Limits::default(),
    )?)
}
fn dynamics() -> Dynamics {
    Dynamics {
        axis_mask: 5,
        interpolation_period_ns: 1_000_000,
        trajectory: AxisDynamics {
            velocity_mm_s: 10000.0,
            acceleration_mm_s2: 10000.0,
            jerk_mm_s3: 10000.0,
        },
        scalar_origin_mm: 0.0,
        axes: [AxisDynamics {
            velocity_mm_s: 20.0,
            acceleration_mm_s2: 300.0,
            jerk_mm_s3: 3000.0,
        }; 3],
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10 * a.abs().max(1.0), "{a} != {b}");
}

#[test]
fn mixed_feed_css_and_radius_units_survive_tool_work_and_spindle_changes() -> TestResult {
    use sha2::{Digest, Sha256};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("spindle-fixtures/manifest.json"))?;
    let generator = std::fs::read(root.join("../../tools/native-spindle-fixtures/generate.cjs"))?;
    assert_eq!(
        format!("{:x}", Sha256::digest(generator)),
        manifest["generatorSHA256"]
    );
    for case in manifest["cases"].as_array().ok_or("cases")? {
        let name = case["name"].as_str().ok_or("name")?;
        for (suffix, key) in [("stpnc", "sourceSHA256"), ("plan.json", "setupSHA256")] {
            assert_eq!(
                format!(
                    "{:x}",
                    Sha256::digest(std::fs::read(
                        root.join(format!("tests/spindle-fixtures/{name}.{suffix}"))
                    )?)
                ),
                case[key]
            );
        }
        let p = fixture(name)?;
        let b = bind(&p, &snapshot())?;
        let l = lower(&b, dynamics())?;
        nextnc_task::steps::Layout::from_lowered(&p, &b, &l)?;
        let mut sync = 0.0;
        let mut per_rev = 0;
        let mut per_second = 0;
        let mut arcs = 0;
        let mut css = 0;
        for piece in l.pieces() {
            match piece.payload {
                Payload::State(BoundAction::State(Action::RestoreFeedPerMinute)) => sync = 0.0,
                Payload::SpindleSync { mm_per_rev } => sync = mm_per_rev,
                Payload::State(BoundAction::Css(demand)) => {
                    close(demand.x_offset_mm, if css < 2 { 11.7 } else { 18.2 });
                    close(
                        demand.surface_mm_s,
                        if css % 2 == 0 {
                            80000.0 / 60.0
                        } else {
                            40000.0 / 60.0
                        },
                    );
                    close(
                        demand.factor_rpm_mm(),
                        if css % 2 == 0 { 80000.0 } else { 40000.0 } / std::f64::consts::TAU,
                    );
                    assert_eq!(demand.maximum_rpm, 1800.0);
                    assert_eq!(
                        demand.clockwise,
                        case["clockwise"].as_bool().ok_or("direction")?
                    );
                    css += 1;
                }
                _ => (),
            }
            if let Payload::Motion {
                motion, dynamics, ..
            } = piece.payload
            {
                match motion.feed {
                    Feed::PerRevolution {
                        mm_per_rev,
                        spindle,
                    } => {
                        close(sync, mm_per_rev);
                        assert_eq!(spindle, 0);
                        close(mm_per_rev, if per_rev % 5 == 4 { 0.3 } else { 0.18 });
                        assert_eq!(dynamics.velocity_mm_s, dynamics.maximum_velocity_mm_s);
                        assert!(
                            dynamics.velocity_mm_s > mm_per_rev * 10.0,
                            "ceiling must not be a one-time nominal spindle conversion"
                        );
                        per_rev += 1;
                        if motion.circular.is_some() {
                            arcs += 1;
                        }
                    }
                    Feed::PerSecond(rate) => {
                        assert_eq!(sync, 0.0);
                        close(rate, 2.0);
                        per_second += 1;
                    }
                    Feed::Rapid => {
                        assert_eq!(sync, 0.0, "rapid must clear hardware synchronization")
                    }
                }
            }
        }
        assert_eq!((per_rev, per_second, arcs, css), (10, 2, 2, 4), "{name}");
        assert_eq!(sync, 0.0, "end must restore ordinary feed mode");
    }
    Ok(())
}

#[test]
fn capability_requires_complete_finite_evidence_and_unshaped_lathe() -> TestResult {
    let p = fixture("lathe-mm-cw-mixed")?;
    for case in 0..15 {
        let mut s = snapshot();
        match case {
            0 => s.spindle = None,
            1 => s.machine = Machine::MillXyz,
            2 => s.shaping = Shaping::EngagedXy,
            3 => s.spindle.as_mut().ok_or("capability")?.css = false,
            4 => s.spindle.as_mut().ok_or("capability")?.identity = [0; 32],
            5 => s.spindle.as_mut().ok_or("capability")?.feedback.maximum_rps = 0.0,
            6 => s.spindle.as_mut().ok_or("capability")?.feedback.maximum_rps = f64::INFINITY,
            7 => {
                s.spindle
                    .as_mut()
                    .ok_or("capability")?
                    .feedback
                    .heartbeat_timeout_s = f64::NAN
            }
            8 => {
                s.spindle
                    .as_mut()
                    .ok_or("capability")?
                    .feedback
                    .comparison_window_s = -0.1
            }
            9 => {
                s.spindle
                    .as_mut()
                    .ok_or("capability")?
                    .feedback
                    .position_error_revs = 0.0
            }
            10 => {
                s.spindle
                    .as_mut()
                    .ok_or("capability")?
                    .feedback
                    .relative_error = 1.0
            }
            11 => {
                s.spindle
                    .as_mut()
                    .ok_or("capability")?
                    .feedback
                    .relative_error = -0.1
            }
            12 => s.maximum_rpm = 1799.0,
            13 => s.work_offsets[0].rotation_degrees = 90.0,
            _ => s.tool_offsets_mm.get_mut(&3).ok_or("tool")?[0] = f64::NAN,
        }
        assert!(bind(&p, &s).is_err(), "invalid capability case {case}");
    }
    let mut s = snapshot();
    s.reverse_spindle = false;
    assert!(bind(&fixture("lathe-mm-ccw-mixed")?, &s).is_err());
    assert!(bind(&p, &s).is_ok());
    Ok(())
}

#[test]
fn g95_with_constant_rpm_still_requires_measured_feedback() -> TestResult {
    let source = include_str!("fixtures/lathe-mm-line.stpnc")
        .replace(
            "'feed speed',NUMERIC_MEASURE(120.),#16",
            "'feed per revolution',NUMERIC_MEASURE(0.18),#18",
        )
        .replace(
            "MACHINING_FEED_SPEED_REPRESENTATION('feed speed'",
            "MACHINING_FEED_SPEED_REPRESENTATION('feed per revolution'",
        )
        .replace(
            "\"dwell\",\"linear\"",
            "\"dwell\",\"feed-per-revolution\",\"linear\"",
        );
    let mut setup: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/lathe-mm-line.plan.json"))?;
    setup["programFingerprint"] = nextnc_native::profile::decode(&source, &Limits::default())?
        .report
        .fingerprint
        .value
        .into();
    let p = compiled::prepare(&source, &serde_json::to_string(&setup)?, &Limits::default())?;
    let mut s = snapshot();
    s.flood = true;
    s.spindle.as_mut().ok_or("capability")?.css = false;
    let bound = bind(&p, &s)?;
    assert!(bound
        .records()
        .iter()
        .any(|r| matches!(r.action,BoundAction::Motion(m)
        if matches!(m.feed,Feed::PerRevolution {mm_per_rev:0.18,spindle:0}))));
    s.spindle = None;
    assert_eq!(
        bind(&p, &s)
            .err()
            .ok_or("G95 accepted without feedback")?
            .reason,
        "spindle synchronization requires qualified feedback capability"
    );
    Ok(())
}

#[test]
fn producer_css_centerline_fixtures_stop_before_independent_offset_changes() -> TestResult {
    use motion_command::{Command, Spindle};
    use sha2::{Digest, Sha256};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("css-fixtures/manifest.json"))?;
    assert_eq!(
        manifest["producer"],
        "b31904b3d5ad9d56cc4cc16b8090e768b0e1958d"
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(
                root.join("../../tools/native-css-fixtures/generate.cjs")
            )?)
        ),
        manifest["generatorSHA256"]
    );
    for case in manifest["cases"].as_array().ok_or("CSS cases")? {
        let name = case["name"].as_str().ok_or("CSS name")?;
        let dir = root.join("tests/css-fixtures");
        for (suffix, key) in [("stpnc", "sourceSHA256"), ("plan.json", "setupSHA256")] {
            assert_eq!(
                format!(
                    "{:x}",
                    Sha256::digest(std::fs::read(dir.join(format!("{name}.{suffix}")))?)
                ),
                case[key]
            );
        }
        let p = compiled::prepare(
            &std::fs::read_to_string(dir.join(format!("{name}.stpnc")))?,
            &std::fs::read_to_string(dir.join(format!("{name}.plan.json")))?,
            &Limits::default(),
        )?;
        let b = bind(&p, &snapshot())?;
        let l = lower(&b, dynamics())?;
        nextnc_task::steps::Layout::from_lowered(&p, &b, &l)?;
        let mut active_css = false;
        let mut demands = Vec::new();
        let mut changes = Vec::new();
        let mut tool_changes = 0;
        let mut cuts = 0;
        let mut arcs = 0;
        for record in b.records() {
            // Source sections change offsets only after a stopped transition.
            // The binder's independent active-CSS update API is tested elsewhere.
            assert!(record.css_update.is_none());
            match record.action {
                BoundAction::Css(demand) => {
                    active_css = true;
                    demands.push((demand.x_offset_mm, demand.surface_mm_s, demand.maximum_rpm));
                    assert_eq!(
                        demand.clockwise,
                        case["clockwise"].as_bool().ok_or("direction")?
                    );
                }
                BoundAction::WorkOffset { index, .. } => {
                    assert!(!active_css, "source work-offset transition must stop CSS");
                    changes.push((index, 0));
                }
                BoundAction::ToolOffset { number, .. } => {
                    assert!(!active_css, "source tool-offset transition must stop CSS");
                    changes.push((0, number));
                }
                BoundAction::State(
                    Action::ResetSpindleDemand | Action::Event(Command::Spindle(Spindle::Stop)),
                ) => active_css = false,
                BoundAction::State(Action::Event(Command::ChangeTool { .. })) => tool_changes += 1,
                BoundAction::Motion(motion) => {
                    if let Feed::PerRevolution {
                        mm_per_rev,
                        spindle,
                    } = motion.feed
                    {
                        assert!(active_css);
                        close(mm_per_rev, 0.18);
                        assert_eq!(spindle, 0);
                        cuts += 1;
                        arcs += usize::from(motion.circular.is_some());
                    }
                }
                _ => (),
            }
        }
        let expected = [
            (11.7, 8000.0, 1800.0),
            (11.7, 8000.0, 900.0),
            (12.2, 6000.0, 1200.0),
            (12.2, 6000.0, 600.0),
            (18.2, 10000.0, 1600.0),
            (18.2, 10000.0, 800.0),
        ];
        assert_eq!(demands.len(), expected.len());
        for (actual, wanted) in demands.iter().zip(expected) {
            close(actual.0, wanted.0);
            close(actual.1, wanted.1 / 60.0);
            close(actual.2, wanted.2);
        }
        assert_eq!((tool_changes, cuts, arcs), (2, 15, 3));
        assert_eq!(
            changes,
            vec![
                (0, 0),
                (1, 0),
                (0, 2),
                (1, 0),
                (0, 3),
                (0, 0),
                (2, 0),
                (0, 3),
                (0, 0)
            ]
        );
        assert!(!active_css, "normal completion must clear CSS");
    }
    Ok(())
}
