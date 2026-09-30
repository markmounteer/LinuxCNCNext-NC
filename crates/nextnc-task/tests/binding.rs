use motion_command::{Machine, Plane};
use nextnc_native::{
    compiled::{self, Action, PreparedPlan},
    part21::Limits,
};
use nextnc_task::binding::*;
use std::collections::BTreeMap;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture(name: &str) -> Result<PreparedPlan, Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    Ok(compiled::prepare(
        &std::fs::read_to_string(root.join(format!("{name}.stpnc")))?,
        &std::fs::read_to_string(root.join(format!("{name}.plan.json")))?,
        &Limits::default(),
    )?)
}
fn snapshot(machine: Machine) -> Snapshot {
    let mill = machine == Machine::MillXyz;
    let work = WorkOffset {
        translation_mm: [
            11.0,
            if mill { 12.0 } else { 0.0 },
            -4.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ],
        rotation_degrees: 0.0,
    };
    Snapshot {
        machine,
        commanded_pose_mm: [
            15.0,
            if mill { 7.0 } else { 0.0 },
            21.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ],
        active_work_offset: 1,
        work_offsets: [work; 9],
        temporary_offset_mm: [
            4.0,
            if mill { 2.0 } else { 0.0 },
            -7.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ],
        active_tool_offset_mm: [9.0, 0.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        tool_offsets_mm: BTreeMap::from([
            (1, [99.0, 0.0, 17.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            (
                2,
                [
                    0.7,
                    if mill { 0.4 } else { 0.0 },
                    0.3,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                ],
            ),
        ]),
        limits: [AxisLimits {
            minimum_mm: -2000.0,
            maximum_mm: 2000.0,
        }; 3],
        shaping: Shaping::Disabled,
        reverse_spindle: false,
        maximum_rpm: 2000.0,
        flood: true,
        mist: true,
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}

#[test]
fn complete_mm_inch_xyz_xz_matrix_keeps_same_table_and_independent_t_h_mapping() -> TestResult {
    let manifest: serde_json::Value = serde_json::from_str(include_str!("fixtures/manifest.json"))?;
    let cases = manifest["cases"].as_array().ok_or("cases")?;
    assert_eq!(cases.len(), 26);
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let generator = std::fs::read(root.join("../../tools/native-task-fixtures/generate.cjs"))?;
    assert_eq!(
        format!("{:x}", Sha256::digest(generator)),
        manifest["generatorSHA256"]
    );
    for c in cases {
        let name = c["name"].as_str().ok_or("name")?;
        for (suffix, key) in [("stpnc", "sourceSHA256"), ("plan.json", "setupSHA256")] {
            let bytes = std::fs::read(root.join(format!("tests/fixtures/{name}.{suffix}")))?;
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                c[key],
                "{name} {key}"
            );
        }
        let plan = fixture(name)?;
        let s = snapshot(if c["machine"] == "mill" {
            Machine::MillXyz
        } else {
            Machine::LatheXz
        });
        let b = bind(&plan, &s)?;
        assert_eq!(b.records().len(), plan.commands().len());
        assert_eq!(b.initial_pose_mm(), s.commanded_pose_mm);
        let factor = if c["units"] == "inch" { 25.4 } else { 1.0 };
        let mut count = 0;
        for (i, r) in b.records().iter().enumerate() {
            assert_eq!(r.command, i);
            assert_eq!(r.source, plan.commands()[i]);
            if matches!(r.source.action, Action::Motion(_)) {
                count += 1;
                let BoundAction::Motion(m) = r.action else {
                    return Err("lost motion".into());
                };
                for axis in 0..3 {
                    // Independent expected transform: G92 was explicitly reset;
                    // physical tool 1 is NOT the independently selected H2 row.
                    let expected = c["start"][axis].as_f64().ok_or("start")? * factor
                        + s.work_offsets[0].translation_mm[axis]
                        + s.tool_offsets_mm[&2][axis];
                    close(m.start_mm[axis], expected);
                    close(
                        m.end_mm[axis],
                        c["end"][axis].as_f64().ok_or("end")? * factor
                            + s.work_offsets[0].translation_mm[axis]
                            + s.tool_offsets_mm[&2][axis],
                    );
                }
                assert!(m.start_mm[3..].iter().all(|v| *v == 0.0));
                if let Some(arc) = m.circular {
                    for axis in 0..3 {
                        close(
                            arc.center_mm[axis],
                            c["center"][axis].as_f64().ok_or("center")? * factor
                                + s.work_offsets[0].translation_mm[axis]
                                + s.tool_offsets_mm[&2][axis],
                        );
                    }
                    close(
                        arc.sweep_radians,
                        c["sweepRadians"].as_f64().ok_or("sweep")?,
                    );
                }
            }
        }
        assert_eq!(count, 1);
        let end = b.final_pose_mm();
        if s.machine == Machine::MillXyz {
            close(end[0], 0.0);
            close(end[1], 0.0);
            close(end[2], 20.0 * factor);
        } else {
            close(end[0], 25.0 * factor);
            close(end[1], 0.0);
            close(end[2], 10.0 * factor);
        }
    }
    Ok(())
}

#[test]
fn complete_path_limits_include_arc_interior_and_translated_work_tool_offsets() -> TestResult {
    let plan = fixture("mill-mm-full-xy")?;
    let mut s = snapshot(Machine::MillXyz);
    // The full circle starts/ends at machine X=16.7, Y=16.4. Its interior
    // reaches Y=18.4; every reviewed setup/end point stays below this limit.
    s.limits[1].maximum_mm = 17.5;
    let e = bind(&plan, &s).err().ok_or("interior violation accepted")?;
    assert_eq!(
        e.reason,
        "continuous machine path exceeds live travel limits"
    );
    assert!(e.command.is_some());
    s.limits[1].maximum_mm = 18.4;
    assert!(bind(&plan, &s).is_ok());
    s.tool_offsets_mm.get_mut(&2).ok_or("offset")?[1] += 0.1;
    assert!(bind(&plan, &s).is_err());
    Ok(())
}

#[test]
fn shaper_modes_allow_xy_or_pure_z_but_refuse_mixed_and_helical_jobs_before_output() -> TestResult {
    for name in ["mill-mm-arc-xy", "mill-mm-full-xy", "mill-mm-pure-z"] {
        let mut s = snapshot(Machine::MillXyz);
        s.shaping = Shaping::EngagedXy;
        assert!(bind(&fixture(name)?, &s).is_ok(), "{name}");
    }
    for name in [
        "mill-mm-line",
        "mill-mm-arc-xz",
        "mill-mm-arc-yz",
        "mill-mm-helix-xy",
        "mill-mm-multi-xy",
    ] {
        let mut s = snapshot(Machine::MillXyz);
        s.shaping = Shaping::EngagedXy;
        assert!(bind(&fixture(name)?, &s).is_err(), "{name}");
        s.shaping = Shaping::Disabled;
        assert!(bind(&fixture(name)?, &s).is_ok(), "{name}");
    }
    Ok(())
}

#[test]
fn work_rotation_is_applied_once_and_unsupported_tilt_is_not_flattened() -> TestResult {
    let mut s = snapshot(Machine::MillXyz);
    s.work_offsets[0].rotation_degrees = 30.0;
    let b = bind(&fixture("mill-mm-arc-xy")?, &s)?;
    let r = b
        .records()
        .iter()
        .find(|r| matches!(r.source.action, Action::Motion(_)))
        .ok_or("source")?;
    let BoundAction::Motion(m) = r.action else {
        return Err("motion".into());
    };
    close(
        m.start_mm[0],
        5.0 * 30f64.to_radians().cos() - 4.0 * 30f64.to_radians().sin() + 11.0 + 0.7,
    );
    close(
        m.start_mm[1],
        5.0 * 30f64.to_radians().sin() + 4.0 * 30f64.to_radians().cos() + 12.0 + 0.4,
    );
    assert!(bind(&fixture("mill-mm-arc-xz")?, &s).is_err());
    s.work_offsets[0].rotation_degrees = 90.0;
    let b = bind(&fixture("mill-mm-arc-xz")?, &s)?;
    let r = b
        .records()
        .iter()
        .find(|r| matches!(r.source.action, Action::Motion(_)))
        .ok_or("source")?;
    let BoundAction::Motion(m) = r.action else {
        return Err("motion".into());
    };
    assert_eq!(m.circular.ok_or("arc")?.plane, Plane::Yz);
    Ok(())
}

#[test]
fn invalid_live_state_never_gets_silently_repaired_or_converted() -> TestResult {
    let plan = fixture("mill-mm-line")?;
    for kind in 0..8 {
        let mut s = snapshot(Machine::MillXyz);
        match kind {
            0 => s.commanded_pose_mm[3] = 1.0,
            1 => s.temporary_offset_mm[3] = 1.0,
            2 => s.active_tool_offset_mm[2] = f64::NAN,
            3 => s.work_offsets[0].rotation_degrees = f64::INFINITY,
            4 => {
                s.tool_offsets_mm.remove(&2);
            }
            5 => s.limits[0].minimum_mm = 3000.0,
            6 => s.maximum_rpm = 500.0,
            _ => s.flood = false,
        }
        assert!(bind(&plan, &s).is_err(), "invalid snapshot case {kind}");
    }
    let mut lathe = snapshot(Machine::LatheXz);
    lathe.commanded_pose_mm[1] = 0.01;
    assert!(bind(&fixture("lathe-mm-line")?, &lathe).is_err());
    lathe.commanded_pose_mm[1] = 0.0;
    lathe.work_offsets[0].rotation_degrees = 90.0;
    assert!(bind(&fixture("lathe-mm-line")?, &lathe).is_err());
    Ok(())
}

#[test]
fn g95_css_remain_explicit_stage4_requirements() -> TestResult {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests-rust/fixtures/benchmark/lathe-mm-arcs-8");
    let plan = compiled::prepare(
        &std::fs::read_to_string(root.with_extension("stpnc"))?,
        &std::fs::read_to_string(root.with_extension("plan.json"))?,
        &Limits::default(),
    )?;
    let e = bind(&plan, &snapshot(Machine::LatheXz))
        .err()
        .ok_or("unsupported synchronization accepted")?;
    assert_eq!(e.reason, "CSS is not qualified in Stage 3");
    assert!(e.command.is_some());
    Ok(())
}

#[test]
fn xy_z_lane_changes_wait_for_real_drain_even_with_available_capacity() -> TestResult {
    use nextnc_task::{lifecycle::*, steps::Layout};

    let plan = fixture("mill-mm-arc-xy")?;
    let mut s = snapshot(Machine::MillXyz);
    s.shaping = Shaping::EngagedXy;
    let bound = bind(&plan, &s)?;
    let points: Vec<_> = bound
        .records()
        .iter()
        .filter(|r| r.drain_before)
        .map(|r| r.command)
        .collect();
    assert!(
        points.len() >= 2,
        "fixture must exercise XY to Z and Z to XY"
    );
    let layout = Layout::from_bound(&plan, &bound)?;
    assert_eq!(layout.groups(), Layout::from_prepared(&plan).groups());
    assert!(Layout::from_bound(&fixture("mill-mm-pure-z")?, &bound).is_err());
    let ready = Readiness {
        automatic: true,
        enabled: true,
        homed: true,
        fault_free: true,
        quiescent: true,
        source_binding_current: true,
        downstream_compatible: true,
    };
    let drained = Drain {
        task_empty: true,
        io_done: true,
        motion_done: true,
        in_position: true,
        shaper_done: true,
        fault_free: true,
        observed_after_admission: true,
    };
    let mut owner = Owner::new(51, 4096)?;
    let generation = owner.begin_select()?;
    owner.selected(generation, [7; 32], layout)?;
    let binding = Binding {
        generation,
        state_epoch: 1,
        state_sha256: [3; 32],
        artifact_sha256: [7; 32],
    };
    owner.arm(binding, ready)?;
    owner.start(binding, ready, Start::Continuous, 0)?;
    let mut observed = Vec::new();
    let mut seen = Vec::new();
    for _ in 0..plan.commands().len() * 2 {
        if owner.phase() != Phase::Running {
            break;
        }
        if let Some(offer) = owner.offer(4096)? {
            assert!(!points
                .iter()
                .any(|p| offer.commands.start < *p && *p < offer.commands.end));
            seen.extend(offer.commands.clone());
            owner.accept(&offer, offer.commands.end)?;
            owner.admitted(binding, offer.commands.end)?;
        } else {
            let before = owner.prefixes();
            if points.contains(&before.accepted) {
                assert_eq!(
                    owner.drained(
                        binding,
                        Drain {
                            shaper_done: false,
                            ..drained
                        }
                    ),
                    Err(nextnc_task::lifecycle::Error::NotDrained)
                );
                assert_eq!(owner.prefixes(), before);
                assert!(owner.offer(4096)?.is_none());
                observed.push(before.accepted);
            }
            owner.drained(binding, drained)?;
        }
    }
    assert_eq!(seen, (0..plan.commands().len()).collect::<Vec<_>>());
    assert_eq!(observed, points);
    assert_eq!(owner.phase(), Phase::Draining);
    owner.drained(binding, drained)?;
    assert_eq!(owner.phase(), Phase::Reconciling);
    Ok(())
}
