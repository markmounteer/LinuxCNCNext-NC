use super::*;

pub(super) fn shaped() -> Snapshot {
    let mut s = snapshot();
    s.shaping = 1;
    s.kernel = wire::KernelEvidence {
        model: 1,
        axis_mask: 3,
        period_ns: 1_000_000,
        count: 2,
        ..Default::default()
    };
    s.kernel.delays[1] = 100;
    s.kernel.weights[..2].copy_from_slice(&[0.5, 0.5]);
    s
}

#[test]
fn live_kernel_is_required_and_copied_without_normalization() -> TestResult {
    let s = shaped();
    let (decoded, _) = s.decode(&tools())?;
    let kernel = decoded.shaping_kernel.ok_or("missing kernel")?;
    assert_eq!(kernel.terms()[1].delay_ticks, 100);
    assert_eq!(kernel.terms()[1].weight.to_bits(), 0.5_f64.to_bits());
    assert_eq!(std::mem::size_of::<wire::KernelEvidence>(), 400);
    assert_eq!(std::mem::size_of::<Snapshot>(), 1648);
    for case in 0..14 {
        let mut changed = s;
        match case {
            0 => changed.kernel = Default::default(),
            1 => changed.kernel.model = 2,
            2 => changed.kernel.axis_mask = 7,
            3 => changed.kernel.period_ns += 1,
            4 => changed.kernel.count = 33,
            5 => changed.kernel.count = 0,
            6 => changed.kernel.delays[1] = 0,
            7 => changed.kernel.delays[1] = 256,
            8 => changed.kernel.weights[1] = f64::NAN,
            9 => changed.kernel.weights[1] = -0.5,
            10 => changed.kernel.weights[1] = 0.49,
            11 => changed.kernel.delays[2] = 1,
            12 => changed.kernel.weights[2] = -0.0,
            _ => changed.shaping = 0,
        }
        assert!(changed.decode(&tools()).is_err(), "case {case}");
    }
    Ok(())
}

#[test]
fn start_and_resume_refuse_a_different_valid_kernel() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../nextnc-task/tests/fixtures");
    let source = std::fs::read_to_string(root.join("mill-mm-arc-xy.stpnc"))?;
    let setup = std::fs::read_to_string(root.join("mill-mm-arc-xy.plan.json"))?;
    let a = bundle::compile(
        Inputs {
            source: &source,
            setup: &setup,
            tool_table: None,
            target: None,
        },
        &Limits::default(),
    )?;
    let s = shaped();
    let t = tools();
    let mut h = 0;
    assert_eq!(prepare(&a, &s, &t, &mut h), 0);
    for case in 0..4 {
        let mut changed = s;
        match case {
            0 => changed.kernel.delays[1] = 99,
            1 => changed.kernel.weights[..2].copy_from_slice(&[0.4, 0.6]),
            2 => changed.timing.motion_birth[0] += 1,
            _ => {
                changed.shaping = 0;
                changed.kernel = Default::default();
            }
        }
        assert_ne!(
            fresh(&s, &t).configuration,
            fresh(&changed, &t).configuration
        );
        for mode in 0..3 {
            let mut out = Fingerprint::default();
            // SAFETY: all declared extents are backed by live disjoint objects.
            let rc = unsafe {
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
            assert_eq!(rc, -1, "case {case}, mode {mode}");
            assert_eq!(out, Fingerprint::default());
        }
    }
    assert_eq!(nextnc_task_release(h), 0);
    Ok(())
}
