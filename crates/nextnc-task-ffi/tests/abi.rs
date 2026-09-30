use nextnc_native::{
    bundle::{self, Inputs},
    part21::Limits,
};
use nextnc_task_ffi::{
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
