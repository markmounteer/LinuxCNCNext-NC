use nextnc_native::{
    bundle::{self, Inputs},
    part21::Limits,
};
use nextnc_task_ffi::{
    runtime::*,
    wire::{self, Message, Snapshot, Tool},
    *,
};
use std::sync::Mutex;
static SERIAL: Mutex<()> = Mutex::new(());
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn snapshot() -> Snapshot {
    Snapshot {
        abi: wire::ABI,
        bytes: std::mem::size_of::<Snapshot>() as u32,
        machine: 1,
        axis_mask: 7,
        work_offset: 1,
        shaping: 0,
        capabilities: 6,
        reserved: 0,
        pose: [0.0; 9],
        work: [[0.0; 9]; 9],
        rotation: [0.0; 9],
        temporary: [0.0; 9],
        tool_offset: [0.0; 9],
        minimum: [-2000.0; 3],
        maximum: [2000.0; 3],
        velocity: [30.0; 3],
        acceleration: [100.0; 3],
        jerk: [1000.0; 3],
        maximum_rpm: 2000.0,
    }
}
fn artifact() -> Result<bundle::Artifact, Box<dyn std::error::Error>> {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../nextnc-task/tests/fixtures");
    Ok(bundle::compile(
        Inputs {
            source: &std::fs::read_to_string(root.join("mill-mm-multi-xy.stpnc"))?,
            setup: &std::fs::read_to_string(root.join("mill-mm-multi-xy.plan.json"))?,
            tool_table: None,
            target: None,
        },
        &Limits::default(),
    )?)
}
fn tools() -> [Tool; 2] {
    [
        Tool {
            number: 1,
            reserved: 0,
            offset: [0.0; 9],
        },
        Tool {
            number: 2,
            reserved: 0,
            offset: [0.0; 9],
        },
    ]
}
fn prepare(a: &bundle::Artifact, s: &Snapshot, t: &[Tool], h: &mut u64) -> i32 {
    // SAFETY: all input slices and output remain alive, disjoint and correctly sized.
    unsafe {
        nextnc_task_prepare(
            a.bytes().as_ptr(),
            a.bytes().len() as u64,
            s,
            t.as_ptr(),
            t.len() as u64,
            h,
        )
    }
}

fn fresh(s: &Snapshot, t: &[Tool]) -> Fingerprint {
    let mut f = Fingerprint::default();
    // SAFETY: every input and output is a live, disjoint, correctly sized object.
    assert_eq!(
        // SAFETY: all declared extents are backed by the live objects above.
        unsafe {
            nextnc_task_fingerprint(
                s,
                t.as_ptr(),
                t.len() as u64,
                &mut f,
                std::mem::size_of::<Fingerprint>() as u64,
            )
        },
        0
    );
    f
}
fn state(owner: u64) -> Status {
    let mut s = Status::default();
    // SAFETY: s supplies the exact declared writable extent.
    assert_eq!(
        // SAFETY: s is aligned and matches the declared byte size.
        unsafe { nextnc_owner_status(owner, &mut s, std::mem::size_of::<Status>() as u64) },
        0
    );
    s
}
fn next(owner: u64) -> Dispatch {
    let mut d = Dispatch::default();
    // SAFETY: d supplies the exact declared writable extent.
    assert_eq!(
        // SAFETY: d is aligned and matches the declared byte size.
        unsafe { nextnc_owner_next(owner, &mut d, std::mem::size_of::<Dispatch>() as u64) },
        0
    );
    d
}
fn owner_selection(candidate: u64) -> (u64, u64) {
    let mut owner = 0;
    let mut selection = 0;
    // SAFETY: both outputs are aligned, writable u64 values.
    unsafe {
        assert_eq!(nextnc_owner_create(3, &mut owner), 0);
        assert_eq!(nextnc_owner_begin(owner, &mut selection), 0);
    }
    assert_eq!(nextnc_owner_attach(owner, selection, candidate), 0);
    (owner, selection)
}
fn start(owner: u64, f: &Fingerprint, mode: u32, flags: u32, restart: u64) -> i32 {
    // SAFETY: f remains a valid immutable fingerprint for the entire call.
    unsafe { nextnc_owner_start(owner, f, 1, flags, mode, restart) }
}
fn procedure(owner: u64) -> Procedure {
    let mut p = Procedure::default();
    // SAFETY: p has the exact declared writable extent.
    assert_eq!(
        // SAFETY: p remains aligned and valid for the stated byte count.
        unsafe { nextnc_owner_procedure(owner, &mut p, std::mem::size_of::<Procedure>() as u64) },
        0
    );
    p
}
fn rebound(p: Procedure, data: &[u8], s: &Snapshot, t: &[Tool], out: &mut u64) -> i32 {
    // SAFETY: all input/output extents are live, disjoint and correctly sized.
    unsafe {
        nextnc_task_rebind(
            p.candidate,
            p.completed,
            data.as_ptr(),
            data.len() as u64,
            s,
            t.as_ptr(),
            t.len() as u64,
            out,
        )
    }
}
fn adopt(owner: u64, p: Procedure, candidate: u64, f: &Fingerprint, tool: u32, tick: u64) -> i32 {
    // SAFETY: f is immutable and valid for the complete synchronous call.
    unsafe { nextnc_owner_rebind(owner, p.selection, p.serial, candidate, f, tool, 63, tick) }
}
fn observe(s: &mut Snapshot, m: Message) {
    match m.kind {
        1..=3 => s.pose = m.end,
        11 => s.temporary = [0.0; 9],
        13 => {
            s.work_offset = m.argument as u32;
            s.work[(m.argument - 1) as usize] = m.end;
            s.rotation[(m.argument - 1) as usize] = m.value;
        }
        14 => s.tool_offset = m.end,
        _ => (),
    }
}

#[test]
fn owner_dispatch_uses_actual_results_and_complete_drains_before_mdi() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let a = artifact()?;
    let mut candidate = 0;
    assert_eq!(prepare(&a, &snapshot(), &tools(), &mut candidate), 0);
    let (owner, selection) = owner_selection(candidate);
    assert_eq!(nextnc_task_release(candidate), -1); // live owner pins the allocation
    assert_eq!(start(owner, &fresh(&snapshot(), &tools()), 0, 127, 0), 0);
    let mut tick = 1;
    let mut pieces = 0;
    let mut commands = 0;
    let mut circles = 0;
    let mut live = snapshot();
    let mut table = tools();
    let mut rebinds = 0;
    while state(owner).phase != 9 {
        assert!(tick < 500, "owner stopped making progress");
        let d = next(owner);
        if d.serial == 0 {
            let p = procedure(owner);
            let mut replacement = 0;
            if p.serial != 0 {
                assert_eq!(next(owner).serial, 0); // no suffix escapes the procedure
                let mut changed_bytes = a.bytes().to_vec();
                changed_bytes[0] ^= 1;
                assert_eq!(
                    rebound(p, &changed_bytes, &live, &table, &mut replacement),
                    -1
                );
                assert_eq!(replacement, 0);
                let mut outside = table;
                outside[1].offset[2] = 1e6;
                assert_eq!(rebound(p, a.bytes(), &live, &outside, &mut replacement), -1);
                assert_eq!(replacement, 0); // complete suffix limit check, not just T validity
                table[1].offset[2] = 0.3; // physical T1 and independent H2 stay distinct
                assert_eq!(rebound(p, a.bytes(), &live, &table, &mut replacement), 0);
                assert_eq!(
                    adopt(owner, p, replacement, &fresh(&live, &table), p.tool, tick),
                    -1
                ); // undrained
            }
            // A new observation and all six actual completion domains are needed.
            assert_eq!(nextnc_owner_control(owner, 5, 0, 47, tick), -1);
            assert_eq!(nextnc_owner_control(owner, 5, 0, 63, tick), 0);
            if p.serial != 0 {
                assert_eq!(
                    adopt(owner, p, replacement, &fresh(&live, &table), 99, tick),
                    -1
                );
                let mut changed = live;
                changed.pose[2] += 0.01;
                assert_eq!(
                    adopt(
                        owner,
                        p,
                        replacement,
                        &fresh(&changed, &table),
                        p.tool,
                        tick
                    ),
                    -1
                );
                assert_eq!(
                    adopt(
                        owner,
                        Procedure {
                            serial: p.serial + 1,
                            ..p
                        },
                        replacement,
                        &fresh(&live, &table),
                        p.tool,
                        tick
                    ),
                    -1
                );
                assert_eq!(next(owner).serial, 0);
                assert_eq!(
                    adopt(owner, p, replacement, &fresh(&live, &table), p.tool, tick),
                    0
                );
                assert_eq!(nextnc_owner_result(owner, selection, p.serial, 0, tick), -1); // old receipt loses authority
                assert_eq!(
                    adopt(owner, p, replacement, &fresh(&live, &table), p.tool, tick),
                    -1
                );
                assert_eq!(nextnc_task_release(candidate), 0);
                candidate = replacement;
                rebinds += 1;
            }
        } else {
            let retry = next(owner);
            assert_eq!(retry.message, d.message);
            assert_eq!(retry.serial, d.serial);
            assert_eq!(d.selection, selection);
            assert_eq!(d.serial, pieces + 1);
            assert_eq!(d.message.command, commands);
            assert_eq!(state(owner).admitted, commands);
            assert_eq!(nextnc_owner_issue(owner, selection, d.serial, tick), 0);
            assert_eq!(nextnc_owner_issue(owner, selection, d.serial, tick), -1);
            assert_eq!(nextnc_owner_result(owner, selection, d.serial, 0, tick), 0);
            assert_eq!(nextnc_owner_result(owner, selection, d.serial, 0, tick), 0);
            // Same-cycle status is too old even if every completion bit is true.
            assert_eq!(nextnc_owner_control(owner, 5, 0, 63, tick), -1);
            pieces += 1;
            if d.message.flags & 8 != 0 {
                commands += 1;
            }
            if d.message.kind == 2 {
                circles += 1;
                assert!((d.message.end[2] - 5.8).abs() < 1e-8); // Z5 + 0.5 mm source rise + new H2
            }
            observe(&mut live, d.message);
        }
        tick += 1;
    }
    assert_eq!(commands as usize, a.prepared().commands().len());
    assert_eq!(circles, 1);
    assert_eq!(rebinds, 1);
    let s = state(owner);
    assert_eq!(s.completed, commands);
    assert_eq!(s.accepted_pieces, pieces);
    assert_eq!(s.allows_mdi, 0);
    assert_eq!(nextnc_owner_control(owner, 9, 0, 63, tick), -1);
    assert_eq!(nextnc_owner_control(owner, 9, 1, 63, tick), 0);
    assert_eq!(state(owner).phase, 10);
    assert_eq!(state(owner).allows_mdi, 1);
    assert_eq!(nextnc_owner_destroy(owner), 0);
    assert_eq!(nextnc_task_release(candidate), 0);
    Ok(())
}

#[test]
fn held_procedure_adoption_and_late_worker_abort_keep_the_same_prefix() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    for abort_worker in [false, true] {
        let a = artifact()?;
        let mut candidate = 0;
        let mut live = snapshot();
        let t = tools();
        assert_eq!(prepare(&a, &live, &t, &mut candidate), 0);
        let (owner, selection) = owner_selection(candidate);
        assert_eq!(start(owner, &fresh(&live, &t), 0, 127, 0), 0);
        let mut tick = 1;
        let p = loop {
            assert!(tick < 500);
            let p = procedure(owner);
            if p.serial != 0 {
                break p;
            }
            let d = next(owner);
            if d.serial == 0 {
                assert_eq!(nextnc_owner_control(owner, 5, 0, 63, tick), 0);
            } else {
                assert_eq!(nextnc_owner_issue(owner, selection, d.serial, tick), 0);
                assert_eq!(nextnc_owner_result(owner, selection, d.serial, 0, tick), 0);
                observe(&mut live, d.message);
            }
            tick += 1;
        };
        assert_eq!(nextnc_owner_control(owner, 5, 0, 63, tick), 0);
        let mut replacement = 0;
        assert_eq!(rebound(p, a.bytes(), &live, &t, &mut replacement), 0);
        assert_eq!(nextnc_owner_control(owner, 1, 0, 0, tick), 0);
        assert_eq!(nextnc_owner_control(owner, 2, 1, 0, tick + 1), 0);
        tick += 2;
        if abort_worker {
            assert_eq!(nextnc_owner_control(owner, 6, 0, 0, tick), 0);
            assert_eq!(
                adopt(owner, p, replacement, &fresh(&live, &t), p.tool, tick),
                -1
            );
            assert_eq!(next(owner).serial, 0);
        } else {
            assert_eq!(
                adopt(owner, p, replacement, &fresh(&live, &t), p.tool, tick),
                0
            );
            assert_eq!(state(owner).phase, 6);
            assert_eq!(state(owner).completed, p.completed);
            assert_eq!(next(owner).serial, 0); // adoption does not release the hold
            assert_eq!(nextnc_owner_control(owner, 4, 0, 0, tick), 0);
            let end = state(owner).proposed_step_end;
            assert_eq!(nextnc_owner_control(owner, 3, end, 127, tick), 0);
            let d = next(owner);
            assert_eq!(d.message.command, p.completed); // neither skip nor replay
            assert!(d.serial > p.serial);
            assert_eq!(nextnc_owner_result(owner, selection, p.serial, 0, tick), -1);
            assert_eq!(nextnc_owner_control(owner, 6, 0, 0, tick), 0);
        }
        assert_eq!(nextnc_owner_control(owner, 9, 1, 63, tick + 1), 0);
        let mut newer = 0;
        // SAFETY: newer is a disjoint writable u64.
        assert_eq!(unsafe { nextnc_owner_begin(owner, &mut newer) }, 0);
        // Worker suffixes cannot be attached as new jobs with an old prefix.
        assert_eq!(nextnc_owner_attach(owner, newer, replacement), -1);
        assert_eq!(nextnc_owner_failed(owner, newer), 0);
        assert_eq!(nextnc_owner_destroy(owner), 0);
        assert_eq!(nextnc_task_release(candidate), 0);
        assert_eq!(nextnc_task_release(replacement), 0);
    }
    Ok(())
}

#[test]
fn live_fingerprint_stale_workers_and_competing_threads_cannot_start_old_work() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let a = artifact()?;
    let mut candidate = 0;
    let s = snapshot();
    let t = tools();
    assert_eq!(prepare(&a, &s, &t, &mut candidate), 0);
    let (owner, selection) = owner_selection(candidate);
    assert_eq!(start(owner, &fresh(&s, &t), 0, 127, 1), -1);
    for flag in 0..7 {
        assert_eq!(start(owner, &fresh(&s, &t), 0, 127 ^ (1 << flag), 0), -1);
    }
    for kind in 0..8 {
        let mut changed = s;
        let mut changed_tools = t;
        match kind {
            0 => changed.pose[0] = 0.1,
            1 => changed.work[0][2] = 0.1,
            2 => changed.tool_offset[2] = 0.1,
            3 => changed.temporary[2] = 0.1,
            4 => changed.velocity[0] *= 0.5,
            5 => changed.shaping = 1,
            6 => changed_tools[1].offset[2] = 0.1,
            _ => changed.rotation[0] = 90.0,
        }
        assert_eq!(
            start(owner, &fresh(&changed, &changed_tools), 0, 127, 0),
            -1
        );
    }
    assert_eq!(fresh(&s, &[t[1], t[0]]), fresh(&s, &t));
    assert_eq!(
        std::thread::spawn(move || nextnc_owner_attach(owner, selection, candidate))
            .join()
            .map_err(|_| "worker panicked")?,
        -1
    );
    let mut newer = 0;
    // SAFETY: newer is a valid u64 output.
    assert_eq!(unsafe { nextnc_owner_begin(owner, &mut newer) }, 0);
    assert!(newer > selection);
    assert_eq!(nextnc_owner_attach(owner, selection, candidate), -1);
    assert_eq!(nextnc_owner_failed(owner, selection), -1);
    assert_eq!(state(owner).phase, 1);
    assert_eq!(nextnc_owner_failed(owner, newer), 0);
    assert_eq!(start(owner, &fresh(&s, &t), 0, 127, 0), -1);
    assert_eq!(nextnc_owner_destroy(owner), 0);
    assert_eq!(nextnc_task_release(candidate), 0);
    Ok(())
}

#[test]
fn hold_step_and_unknown_result_revoke_authority_without_resurrecting_motion() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let a = artifact()?;
    let mut candidate = 0;
    assert_eq!(prepare(&a, &snapshot(), &tools(), &mut candidate), 0);
    let (owner, selection) = owner_selection(candidate);
    assert_eq!(start(owner, &fresh(&snapshot(), &tools()), 0, 127, 0), 0);
    let d = next(owner);
    assert_ne!(d.serial, 0);
    assert_eq!(nextnc_owner_control(owner, 1, 0, 0, 1), 0);
    assert_eq!(next(owner).serial, 0);
    assert_eq!(nextnc_owner_issue(owner, selection, d.serial, 1), -1);
    assert_eq!(nextnc_owner_control(owner, 2, 0, 0, 2), -1);
    assert_eq!(nextnc_owner_control(owner, 2, 1, 0, 2), 0);
    assert_eq!(nextnc_owner_control(owner, 4, 0, 0, 2), 0);
    let end = state(owner).proposed_step_end;
    assert!(end >= state(owner).accepted && end > 0);
    assert_eq!(nextnc_owner_control(owner, 3, end + 1, 127, 2), -1);
    assert_eq!(nextnc_owner_control(owner, 3, end, 127, 2), 0);
    let retry = next(owner);
    assert_eq!(retry.serial, d.serial);
    assert_eq!(retry.message, d.message);
    assert_eq!(nextnc_owner_issue(owner, selection, retry.serial, 3), 0);
    assert_eq!(
        nextnc_owner_result(owner, selection, retry.serial, 2, 3),
        -1
    );
    assert_eq!(state(owner).phase, 12);
    assert_eq!(state(owner).allows_mdi, 0);
    assert_eq!(next(owner).serial, 0);
    assert_eq!(nextnc_owner_issue(owner, selection, retry.serial, 4), -1);
    assert_eq!(nextnc_owner_control(owner, 3, 0, 127, 4), -1);
    assert_eq!(nextnc_owner_control(owner, 9, 1, 63, 3), -1);
    assert_eq!(nextnc_owner_control(owner, 9, 1, 63, 5), 0);
    assert_eq!(state(owner).phase, 0);
    assert_eq!(nextnc_owner_destroy(owner), 0);
    assert_eq!(nextnc_task_release(candidate), 0);
    Ok(())
}

#[test]
fn immutable_candidate_reads_keep_all_commands_and_analytic_helix_then_revoke_handle() -> TestResult
{
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let a = artifact()?;
    let mut h = 0;
    assert_eq!(prepare(&a, &snapshot(), &tools(), &mut h), 0);
    assert_ne!(h, 0);
    let mut count = 0;
    // SAFETY: count is a live aligned u64 output.
    assert_eq!(unsafe { nextnc_task_commands(h, &mut count) }, 0);
    assert_eq!(count as usize, a.prepared().commands().len());
    let mut circles = 0;
    let mut terminations = 0;
    for command in 0..count {
        let mut first = Message::default();
        // SAFETY: first is a live, correctly sized output.
        assert_eq!(
            // SAFETY: first is live, aligned and matches the supplied byte size.
            unsafe {
                nextnc_task_piece(
                    h,
                    command,
                    0,
                    &mut first,
                    std::mem::size_of::<Message>() as u64,
                )
            },
            0
        );
        for piece in 0..first.pieces {
            let mut m = Message::default();
            let mut retry = Message::default();
            // SAFETY: both independent outputs are live and match the declared size.
            unsafe {
                assert_eq!(
                    nextnc_task_piece(
                        h,
                        command,
                        piece,
                        &mut m,
                        std::mem::size_of::<Message>() as u64
                    ),
                    0
                );
                assert_eq!(
                    nextnc_task_piece(
                        h,
                        command,
                        piece,
                        &mut retry,
                        std::mem::size_of::<Message>() as u64
                    ),
                    0
                );
            }
            assert_eq!(m, retry);
            assert_eq!(m.command, command);
            assert_eq!(m.piece, piece);
            assert_eq!(m.flags & 8 != 0, piece + 1 == first.pieces);
            if m.kind == 2 {
                circles += 1;
                assert_eq!(m.turn, 2);
                assert_eq!(m.normal, [0.0, 0.0, 1.0]);
                assert_eq!(m.end[2] - m.start[2], 0.5);
            }
            if m.kind == 4 {
                terminations += 1;
                assert_eq!(m.argument, 1);
                assert_eq!(m.value, 0.0);
            }
        }
    }
    assert_eq!(circles, 1);
    assert_eq!(terminations, 1);
    assert_eq!(nextnc_task_release(h), 0);
    count = 99;
    // SAFETY: count is still a valid writable u64; stale handle is an integer only.
    assert_eq!(unsafe { nextnc_task_commands(h, &mut count) }, -1);
    assert_eq!(count, 0);
    assert_eq!(nextnc_task_release(h), -1);
    Ok(())
}

#[test]
fn abi_versions_flags_bounds_and_invalid_live_data_never_publish_a_candidate() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let a = artifact()?;
    for kind in 0..10 {
        let mut s = snapshot();
        let mut t = tools();
        let mut h = 999;
        match kind {
            0 => s.abi += 1,
            1 => s.bytes -= 8,
            2 => s.capabilities |= 8,
            3 => s.reserved = 1,
            4 => s.axis_mask = 5,
            5 => s.pose[3] = 0.1,
            6 => s.jerk[2] = 0.0,
            7 => t[1].number = 1,
            8 => t[1].offset[0] = f64::NAN,
            _ => s.shaping = 1,
        }
        assert_eq!(prepare(&a, &s, &t, &mut h), -1, "{kind}");
        assert_eq!(h, 0);
    }
    let mut h = 88;
    // SAFETY: null pointers and excessive lengths are explicitly rejected before reads.
    unsafe {
        assert_eq!(
            nextnc_task_prepare(
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null(),
                0,
                &mut h
            ),
            -1
        );
        assert_eq!(h, 0);
        assert_eq!(
            nextnc_task_prepare(
                a.bytes().as_ptr(),
                u64::MAX,
                &snapshot(),
                std::ptr::null(),
                0,
                &mut h
            ),
            -1
        );
        assert_eq!(
            nextnc_task_prepare(
                a.bytes().as_ptr(),
                a.bytes().len() as u64,
                &snapshot(),
                std::ptr::null(),
                u64::MAX,
                &mut h
            ),
            -1
        );
    }
    let mut diagnostic = [0u8; 2048];
    // SAFETY: diagnostic buffer has the declared writable extent.
    let length = unsafe { nextnc_task_error(diagnostic.as_mut_ptr(), diagnostic.len() as u64) };
    assert!(length > 1 && length <= diagnostic.len() as u64);
    assert_eq!(diagnostic[length as usize - 1], 0);
    Ok(())
}

#[test]
fn candidate_capacity_and_failed_replacement_do_not_reuse_or_corrupt_a_handle() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let a = artifact()?;
    let mut first = 0;
    let mut second = 0;
    let mut third = 123;
    assert_eq!(prepare(&a, &snapshot(), &tools(), &mut first), 0);
    assert_eq!(prepare(&a, &snapshot(), &tools(), &mut second), 0);
    assert_ne!(first, second);
    assert_eq!(prepare(&a, &snapshot(), &tools(), &mut third), -1);
    assert_eq!(third, 0);
    assert_eq!(nextnc_task_release(first), 0);
    assert_eq!(prepare(&a, &snapshot(), &tools(), &mut third), 0);
    assert!(third > second);
    let mut output = Message::default();
    // SAFETY: output is valid; invalid indexes and sizes must be refused.
    unsafe {
        assert_eq!(
            nextnc_task_piece(
                second,
                u64::MAX,
                0,
                &mut output,
                std::mem::size_of::<Message>() as u64
            ),
            -1
        );
        assert_eq!(
            nextnc_task_piece(
                second,
                0,
                u32::MAX,
                &mut output,
                std::mem::size_of::<Message>() as u64
            ),
            -1
        );
        assert_eq!(nextnc_task_piece(second, 0, 0, &mut output, 1), -1);
    }
    assert_eq!(nextnc_task_release(second), 0);
    assert_eq!(nextnc_task_release(third), 0);
    Ok(())
}

#[test]
fn insufficient_piece_capacity_refuses_before_native_selection_or_motion() -> TestResult {
    let _lock = SERIAL.lock().map_err(|_| "test mutex poisoned")?;
    let artifact = artifact()?;
    let mut candidate = 0;
    assert_eq!(prepare(&artifact, &snapshot(), &tools(), &mut candidate), 0);
    let mut owner = 0;
    let mut selection = 0;
    // SAFETY: both outputs are aligned writable u64 values.
    unsafe {
        assert_eq!(nextnc_owner_create(2, &mut owner), 0);
        assert_eq!(nextnc_owner_begin(owner, &mut selection), 0);
    }
    assert_eq!(nextnc_owner_attach(owner, selection, candidate), -1);
    assert_eq!(state(owner).phase, 1);
    assert_eq!(state(owner).accepted, 0);
    assert_eq!(nextnc_owner_failed(owner, selection), 0);
    assert_eq!(nextnc_owner_destroy(owner), 0);
    assert_eq!(nextnc_task_release(candidate), 0);
    Ok(())
}
