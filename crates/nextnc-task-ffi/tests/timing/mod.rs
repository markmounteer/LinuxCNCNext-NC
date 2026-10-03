use super::*;

pub(super) fn changed_timing(mut s: Snapshot, case: usize) -> Snapshot {
    match case {
        0 => {
            s.timing.servo_period_ns = 500_000;
            s.timing.trajectory_period_ns = 500_000;
            s.timing.cubic_segment_ns = 500_000;
        }
        1 => s.timing.motion_instance += 1,
        _ => s.timing.motion_birth[case - 2] ^= 1,
    }
    s
}

#[test]
fn native_timing_is_required_and_reaches_lowering_without_a_default() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let a = artifact()?;
    let t = tools();
    for ns in [250_000, 500_000, 1_000_000, 2_000_000, 1_234_567] {
        let mut s = snapshot();
        s.timing.servo_period_ns = ns;
        s.timing.trajectory_period_ns = ns;
        s.timing.cubic_segment_ns = ns;
        let (_, dynamics) = s.decode(&t)?;
        assert_eq!(dynamics.interpolation_period_ns, ns);
        let mut h = 0;
        assert_eq!(prepare(&a, &s, &t, &mut h), 0);
        assert_eq!(nextnc_task_release(h), 0);
    }
    for case in 0..10 {
        let mut s = snapshot();
        match case {
            0 => s.timing.model = 0,
            1 => s.timing.model = 2,
            2 => s.timing.servo_period_ns = 0,
            3 => s.timing.trajectory_period_ns += 1,
            4 => s.timing.interpolation_rate = 0,
            5 => s.timing.interpolation_rate = 2,
            6 => s.timing.cubic_segment_ns += 1,
            7 => s.timing.motion_instance = 0,
            8 => s.timing.motion_birth = [0; 4],
            _ => {
                s.timing.trajectory_period_ns = 2_000_000;
                s.timing.interpolation_rate = 2;
            }
        }
        let mut h = 9;
        assert_eq!(prepare(&a, &s, &t, &mut h), -1, "case {case}");
        assert_eq!(h, 0);
    }
    Ok(())
}

#[test]
fn valid_timing_or_motion_birth_changes_invalidate_start_resume_and_tool_checks() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let a = artifact()?;
    let s = snapshot();
    let t = tools();
    let original = fresh(&s, &t);
    let mut h = 0;
    assert_eq!(prepare(&a, &s, &t, &mut h), 0);
    for case in 0..6 {
        let changed = changed_timing(s, case);
        assert_ne!(fresh(&changed, &t).configuration, original.configuration);
        for mode in [0, 1, 2] {
            let mut out = original;
            // SAFETY: immutable, complete inputs and separate valid output.
            let result = unsafe {
                nextnc_task_check_current(
                    h,
                    mode,
                    a.bytes().as_ptr(),
                    a.bytes().len() as u64,
                    &changed,
                    t.as_ptr(),
                    t.len() as u64,
                    &mut out,
                    std::mem::size_of::<Fingerprint>() as u64,
                )
            };
            assert_eq!(result, -1, "case {case}, mode {mode}");
            assert_eq!(out, Fingerprint::default());
        }
    }
    assert_eq!(nextnc_task_release(h), 0);
    Ok(())
}

#[test]
fn both_old_snapshot_abis_refuse_before_reading_the_extended_body() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let a = artifact()?;
    let t = tools();
    for old in [[1_u32, 1096], [2, 1176]] {
        let mut h = 9;
        // SAFETY: only the common eight-byte header is read on ABI mismatch.
        let result = unsafe {
            nextnc_task_prepare(
                a.bytes().as_ptr(),
                a.bytes().len() as u64,
                old.as_ptr().cast(),
                t.as_ptr(),
                t.len() as u64,
                &mut h,
            )
        };
        assert_eq!(result, -1);
        assert_eq!(h, 0);
    }
    assert_eq!(wire::ABI, 3);
    assert_eq!(std::mem::size_of::<wire::TimingEvidence>(), 40);
    assert_eq!(std::mem::offset_of!(Snapshot, timing), 1176);
    Ok(())
}
