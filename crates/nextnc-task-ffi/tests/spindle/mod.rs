use super::*;

#[test]
fn css_offset_update_has_its_own_receipt_piece_without_spindle_restart() -> TestResult {
    let demand = nextnc_task::spindle::CssDemand {
        surface_mm_s: 1000.0,
        maximum_rpm: 1800.0,
        clockwise: false,
        x_offset_mm: 7.5,
    };
    let piece = nextnc_task::lowering::Piece {
        command: 9,
        ordinal: 1,
        payload: nextnc_task::lowering::Payload::CssUpdate(demand),
    };
    let message = wire::encode(&piece, 2, false)?;
    assert_eq!(message.kind, 22);
    assert_eq!(message.command, 9);
    assert_eq!(message.piece, 1);
    assert_eq!(message.flags, wire::CSS | 8);
    assert_eq!(message.argument, -1);
    assert_eq!(message.css_x_offset_mm, 7.5);
    assert_eq!(message.css_maximum_rpm, 1800.0);
    assert_eq!(message.value, 1000.0);
    assert_eq!(message.feed_mm_rev, 0.0);
    for bad in [f64::NAN, f64::INFINITY, f64::MAX, 0.0, -1.0] {
        let invalid = nextnc_task::lowering::Piece {
            payload: nextnc_task::lowering::Payload::CssUpdate(nextnc_task::spindle::CssDemand {
                surface_mm_s: bad,
                ..demand
            }),
            ..piece
        };
        assert!(wire::encode(&invalid, 2, false).is_err());
    }
    Ok(())
}

fn live() -> Snapshot {
    Snapshot {
        machine: 2,
        axis_mask: 5,
        capabilities: 7,
        spindle: wire::SpindleEvidence {
            flags: 3,
            reserved: 0,
            identity: [7; 32],
            maximum_rps: 40.0,
            heartbeat_timeout_s: 0.1,
            comparison_window_s: 0.05,
            position_error_revs: 0.002,
            relative_error: 0.05,
        },
        ..snapshot()
    }
}
fn table() -> [Tool; 3] {
    [
        tools()[0],
        tools()[1],
        Tool {
            number: 3,
            reserved: 0,
            offset: [0.0; 9],
        },
    ]
}
fn job(name: &str) -> Result<bundle::Artifact, Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../nextnc-task/tests/spindle-fixtures");
    Ok(bundle::compile(
        Inputs {
            source: &std::fs::read_to_string(root.join(format!("{name}.stpnc")))?,
            setup: &std::fs::read_to_string(root.join(format!("{name}.plan.json")))?,
            tool_table: None,
            target: None,
        },
        &Limits::default(),
    )?)
}

#[test]
fn spindle_evidence_is_strict_and_every_policy_field_is_bound_for_resume() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "mutex")?;
    let a = job("lathe-mm-cw-mixed")?;
    let s = live();
    let t = table();
    let mut h = 0;
    assert_eq!(prepare(&a, &s, &t, &mut h), 0);
    let original = fresh(&s, &t);
    for case in 0..7 {
        let mut changed = s;
        match case {
            0 => changed.spindle.identity[9] ^= 1,
            1 => changed.spindle.maximum_rps += 1.0,
            2 => changed.spindle.heartbeat_timeout_s += 0.01,
            3 => changed.spindle.comparison_window_s += 0.01,
            4 => changed.spindle.position_error_revs += 0.001,
            5 => changed.spindle.relative_error += 0.01,
            _ => changed.spindle.flags = 1,
        }
        assert_ne!(fresh(&changed, &t).configuration, original.configuration);
        let mut out = Fingerprint::default();
        // SAFETY: input and output extents are live, disjoint Rust values.
        assert_eq!(
            // SAFETY: these disjoint live values supply the declared extents.
            unsafe {
                nextnc_task_check_current(
                    h,
                    1,
                    a.bytes().as_ptr(),
                    a.bytes().len() as u64,
                    &changed,
                    t.as_ptr(),
                    t.len() as u64,
                    &mut out,
                    std::mem::size_of::<Fingerprint>() as u64,
                )
            },
            -1
        );
        assert_eq!(out, Fingerprint::default());
    }
    assert_eq!(nextnc_task_release(h), 0);
    for case in 0..12 {
        let mut changed = s;
        let mut out = 9;
        match case {
            0 => changed.spindle.flags = 0,
            1 => changed.spindle.flags = 2,
            2 => changed.spindle.flags = 7,
            3 => changed.spindle.reserved = 1,
            4 => changed.spindle.identity = [0; 32],
            5 => changed.spindle.maximum_rps = f64::NAN,
            6 => changed.spindle.heartbeat_timeout_s = 0.0,
            7 => changed.spindle.comparison_window_s = f64::INFINITY,
            8 => changed.spindle.position_error_revs = -0.1,
            9 => changed.spindle.relative_error = 1.0,
            10 => {
                changed.machine = 1;
                changed.axis_mask = 7;
            }
            _ => changed.shaping = 1,
        }
        assert_eq!(prepare(&a, &changed, &t, &mut out), -1, "case {case}");
        assert_eq!(out, 0);
    }
    // ABI-1 callers supply only their old common header here. Rejection must
    // happen before any attempted read of the enlarged ABI-2 body.
    let old = [1_u32, 1096];
    let mut out = 9;
    // SAFETY: incompatible header is a complete input for the early-reject path;
    // all other extents are valid and disjoint.
    assert_eq!(
        // SAFETY: ABI mismatch rejects the common header before reading a body.
        unsafe {
            nextnc_task_prepare(
                a.bytes().as_ptr(),
                a.bytes().len() as u64,
                old.as_ptr().cast(),
                t.as_ptr(),
                t.len() as u64,
                &mut out,
            )
        },
        -1
    );
    assert_eq!(out, 0);
    assert_eq!(std::mem::size_of::<Snapshot>(), 1176);
    assert_eq!(std::mem::size_of::<Message>(), 312);
    assert_eq!(std::mem::size_of::<MotionReceipt>(), 72);
    assert_eq!(std::mem::offset_of!(Snapshot, spindle), 1096);
    Ok(())
}

#[test]
fn native_mixed_feeds_keep_recoverable_units_through_rebind_and_abort() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "mutex")?;
    for name in ["lathe-mm-cw-mixed", "lathe-inch-ccw-mixed"] {
        for abort_in_rapid in [false, true] {
            let a = job(name)?;
            let mut s = live();
            let mut t = table();
            let mut candidate = 0;
            assert_eq!(prepare(&a, &s, &t, &mut candidate), 0);
            let (owner, selection) = owner_selection(candidate);
            assert_eq!(start(owner, &fresh(&s, &t), 0, 127, 0), 0);
            let mut tick = 1;
            let mut modal = (0, 0.0, 0.0);
            let mut sync = 0.0;
            let mut per_rev = 0;
            let mut css = 0;
            let mut rebinds = 0;
            let mut aborted = false;
            while state(owner).phase != 9 {
                assert!(tick < 1000, "no progress");
                let d = next(owner);
                if d.serial == 0 {
                    let p = procedure(owner);
                    assert_eq!(nextnc_owner_control(owner, 5, 0, 63, tick), 0);
                    if p.serial != 0 {
                        if p.tool == 2 {
                            t[2].offset[0] = 0.5;
                        }
                        let mut replacement = 0;
                        assert_eq!(rebound(p, a.bytes(), &s, &t, &mut replacement), 0);
                        assert_eq!(
                            adopt(owner, p, replacement, &fresh(&s, &t), p.tool, tick),
                            0
                        );
                        assert_eq!(nextnc_task_release(candidate), 0);
                        candidate = replacement;
                        rebinds += 1;
                    }
                } else {
                    assert_eq!(nextnc_owner_issue(owner, selection, d.serial, tick), 0);
                    assert_eq!(nextnc_owner_result(owner, selection, d.serial, 0, tick), 0);
                    let m = d.message;
                    match m.kind {
                        5 => {
                            assert_eq!(d.recipient, 3);
                            sync = m.feed_mm_rev;
                        }
                        15 => {
                            sync = 0.0;
                            modal = (0, 0.0, 0.0);
                        }
                        17 if m.flags & wire::CSS != 0 => {
                            css += 1;
                            assert_eq!(d.recipient, 3);
                            assert_eq!(m.css_maximum_rpm, 1800.0);
                            assert_eq!(m.css_x_offset_mm, if css <= 2 { 0.0 } else { 0.5 });
                            assert!(
                                (m.css_factor_rpm_mm - m.value * 60.0 / std::f64::consts::TAU)
                                    .abs()
                                    < 1e-9
                            );
                        }
                        1..=3 => {
                            if m.flags & 1 == 0 {
                                if m.flags & wire::FEED_PER_REV != 0 {
                                    modal = (1, 0.0, m.feed_mm_rev);
                                    per_rev += 1;
                                    assert_eq!(m.feed_mm_s, 0.0);
                                    assert_eq!(sync, m.feed_mm_rev);
                                    assert_eq!(m.velocity, m.maximum_velocity);
                                } else {
                                    modal = (0, m.feed_mm_s, 0.0);
                                    assert_eq!(sync, 0.0);
                                }
                            } else {
                                assert_eq!(sync, 0.0);
                            }
                            if m.kind != 3 {
                                let (code, receipt) = motion_receipt(owner, selection, d.serial);
                                assert_eq!(code, 0);
                                assert_eq!(receipt.feed_known, 1);
                                assert_eq!(receipt.reserved, 0);
                                assert_eq!(
                                    (receipt.feed_per_rev, receipt.feed_mm_s, receipt.feed_mm_rev),
                                    modal
                                );
                                if abort_in_rapid && modal.0 == 1 && m.flags & 1 != 0 {
                                    assert_eq!(nextnc_owner_control(owner, 6, 0, 0, tick), 0);
                                    assert_eq!(
                                        motion_receipt(owner, selection, d.serial),
                                        (0, receipt)
                                    );
                                    assert_eq!(next(owner).serial, 0);
                                    assert_eq!(nextnc_owner_control(owner, 9, 1, 63, tick + 1), 0);
                                    aborted = true;
                                    break;
                                }
                            }
                        }
                        _ => (),
                    }
                    observe(&mut s, m);
                }
                tick += 1;
            }
            if abort_in_rapid {
                assert!(aborted);
                assert_eq!(per_rev, 2);
            } else {
                assert_eq!((per_rev, css, rebinds), (10, 4, 2));
                assert_eq!(modal, (0, 0.0, 0.0));
                assert_eq!(sync, 0.0);
                assert_eq!(nextnc_owner_control(owner, 9, 1, 63, tick), 0);
            }
            assert_eq!(state(owner).allows_mdi, 1);
            assert_eq!(nextnc_owner_destroy(owner), 0);
            assert_eq!(nextnc_task_release(candidate), 0);
        }
    }
    Ok(())
}
