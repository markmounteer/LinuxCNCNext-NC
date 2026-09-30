//! In-process C ABI for preparing and reading native task messages.
//!
//! Handles identify immutable candidates, not permission to execute. The owning
//! LinuxCNC task must still enforce lifecycle, fresh binding, queue receipts and
//! reconciliation. Heavy preparation belongs on a worker, never the task loop.
//! Unsafe code is confined to copying caller-owned C buffers. Every non-null
//! pointer must be valid for the declared extent; inputs and outputs must not
//! overlap, and the caller must not mutate input storage during a call.
#![deny(unsafe_op_in_unsafe_fn)]
pub mod wire;

use nextnc_native::{bundle, part21::Limits};
use nextnc_task::{binding, lowering};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Mutex, OnceLock},
};

const MAX_CANDIDATES: usize = 2;
struct Candidate {
    _artifact: bundle::Artifact,
    _bound: binding::BoundPlan,
    lowered: lowering::Plan,
    _layout: nextnc_task::steps::Layout,
}
#[derive(Default)]
struct Registry {
    next: u64,
    pending: usize,
    candidates: BTreeMap<u64, Candidate>,
}
static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
thread_local! { static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) }; }
type Result<T> = std::result::Result<T, String>;
fn registry() -> std::sync::MutexGuard<'static, Registry> {
    REGISTRY
        .get_or_init(|| Mutex::new(Registry::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}
fn boundary(f: impl FnOnce() -> Result<()>) -> i32 {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => {
            LAST_ERROR.with(|s| s.borrow_mut().clear());
            0
        }
        Ok(Err(message)) => {
            LAST_ERROR.with(|s| *s.borrow_mut() = message.chars().take(1024).collect());
            -1
        }
        Err(_) => {
            LAST_ERROR.with(|s| {
                *s.borrow_mut() = "panic contained at native ABI; candidate not executable".into()
            });
            -2
        }
    }
}
fn address<T>(pointer: *const T) -> Result<()> {
    if pointer.is_null() || !(pointer as usize).is_multiple_of(std::mem::align_of::<T>()) {
        Err("null or misaligned C buffer".into())
    } else {
        Ok(())
    }
}

struct Reservation;
impl Reservation {
    fn new() -> Result<Self> {
        let mut r = registry();
        if r.pending + r.candidates.len() >= MAX_CANDIDATES {
            return Err("candidate capacity exhausted".into());
        }
        r.pending += 1;
        Ok(Self)
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        registry().pending -= 1;
    }
}

#[no_mangle]
pub extern "C" fn nextnc_task_abi() -> u32 {
    wire::ABI
}

/// Independently decode the entire immutable bundle, bind the whole path, and
/// lower every task message before publishing a handle. Output is zero on error.
/// # Safety
/// Input pointers must reference valid immutable nonoverlapping storage for the
/// declared extents. `output` must be aligned and writable for one u64. Snapshot
/// has the exact layout/size of the public header. Null tools is allowed at zero count.
#[no_mangle]
pub unsafe extern "C" fn nextnc_task_prepare(
    bytes: *const u8,
    length: u64,
    snapshot: *const wire::Snapshot,
    tools: *const wire::Tool,
    tool_count: u64,
    output: *mut u64,
) -> i32 {
    boundary(|| {
        address(output)?;
        // SAFETY: output extent/alignment is the caller contract, checked for null/alignment.
        unsafe {
            output.write(0);
        }
        let limits = Limits::default();
        let length = usize::try_from(length).map_err(|_| "bundle length overflows host")?;
        let count = usize::try_from(tool_count).map_err(|_| "tool count overflows host")?;
        if length == 0 || length > limits.bundle_bytes || count > wire::MAX_TOOLS {
            return Err("bundle or tool table exceeds ABI bounds".into());
        }
        address(bytes)?;
        address(snapshot)?;
        if count > 0 {
            address(tools)?;
        }
        // Check the common eight-byte header before reading the versioned body.
        // SAFETY: every snapshot call must provide at least the common header.
        let header = unsafe { (snapshot.cast::<[u32; 2]>()).read() };
        if header != [wire::ABI, std::mem::size_of::<wire::Snapshot>() as u32] {
            return Err("unsupported snapshot ABI/size".into());
        }
        let reservation = Reservation::new()?;
        // SAFETY: caller supplies valid immutable extents; sizes are bounded and aligned.
        let (snapshot, tools, bytes) = unsafe {
            let s = snapshot.read();
            let t = if count == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(tools, count).to_vec()
            };
            (s, t, std::slice::from_raw_parts(bytes, length).to_vec())
        };
        let (snapshot, dynamics) = snapshot.decode(&tools)?;
        let artifact = bundle::load(bytes, &limits).map_err(|e| e.to_string())?;
        let bound = binding::bind(artifact.prepared(), &snapshot).map_err(|e| e.to_string())?;
        let lowered = lowering::lower(&bound, dynamics).map_err(|e| e.to_string())?;
        let layout =
            nextnc_task::steps::Layout::from_lowered(artifact.prepared(), &bound, &lowered)?;
        // Exercise the complete wire conversion before exposing any candidate.
        for piece in lowered.pieces() {
            wire::encode(
                piece,
                lowered.commands()[piece.command].len(),
                lowered
                    .drains_before()
                    .binary_search(&piece.command)
                    .is_ok(),
            )?;
        }
        let handle = {
            let mut r = registry();
            let handle = r
                .next
                .checked_add(1)
                .ok_or("native handle space exhausted")?;
            r.next = handle;
            r.candidates.insert(
                handle,
                Candidate {
                    _artifact: artifact,
                    _bound: bound,
                    lowered,
                    _layout: layout,
                },
            );
            handle
        };
        drop(reservation);
        // SAFETY: output remains valid for this synchronous call; inputs have been copied.
        unsafe {
            output.write(handle);
        }
        Ok(())
    })
}

/// # Safety
/// `output` must be aligned and writable for one u64, disjoint from other buffers.
#[no_mangle]
pub unsafe extern "C" fn nextnc_task_commands(handle: u64, output: *mut u64) -> i32 {
    boundary(|| {
        address(output)?;
        // SAFETY: valid output extent is supplied by caller.
        unsafe {
            output.write(0);
        }
        let r = registry();
        let c = r.candidates.get(&handle).ok_or("stale candidate handle")?;
        // SAFETY: as above; the value has no pointers or Rust-owned allocation.
        unsafe {
            output.write(c.lowered.commands().len() as u64);
        }
        Ok(())
    })
}

/// Return one checked piece; repeat reads are immutable and have no execution side effect.
/// # Safety
/// `output` must be aligned and writable for `output_size` bytes and must not alias input.
#[no_mangle]
pub unsafe extern "C" fn nextnc_task_piece(
    handle: u64,
    command: u64,
    piece: u32,
    output: *mut wire::Message,
    output_size: u64,
) -> i32 {
    boundary(|| {
        if output_size != std::mem::size_of::<wire::Message>() as u64 {
            return Err("unsupported output ABI size".into());
        }
        address(output)?;
        // SAFETY: exact output extent/alignment required above and guaranteed by caller.
        unsafe {
            output.write(wire::Message::default());
        }
        let r = registry();
        let c = r.candidates.get(&handle).ok_or("stale candidate handle")?;
        let index = usize::try_from(command).map_err(|_| "command index overflows host")?;
        let range = c
            .lowered
            .commands()
            .get(index)
            .ok_or("command index outside candidate")?;
        if piece as usize >= range.len() {
            return Err("piece index outside command".into());
        }
        let message = wire::encode(
            &c.lowered.pieces()[range.start + piece as usize],
            range.len(),
            c.lowered.drains_before().binary_search(&index).is_ok(),
        )?;
        // SAFETY: output contract checked above. No Rust references cross the ABI.
        unsafe {
            output.write(message);
        }
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn nextnc_task_release(handle: u64) -> i32 {
    boundary(|| {
        // Remove under lock, drop the potentially large candidate after unlocking.
        let candidate = registry()
            .candidates
            .remove(&handle)
            .ok_or("stale candidate handle")?;
        drop(candidate);
        Ok(())
    })
}

/// Copy this thread's last diagnostic, NUL-terminated. Returns required capacity
/// including the terminator; zero indicates an invalid output buffer.
/// # Safety
/// A nonzero capacity requires valid writable storage of that many bytes.
#[no_mangle]
pub unsafe extern "C" fn nextnc_task_error(output: *mut u8, capacity: u64) -> u64 {
    if capacity > 8192 || (capacity != 0 && output.is_null()) {
        return 0;
    }
    LAST_ERROR.with(|s| {
        let text = s.borrow();
        if capacity > 0 {
            let n = text.len().min(capacity as usize - 1);
            // SAFETY: caller guarantees writable disjoint storage; copy length is bounded.
            unsafe {
                std::ptr::copy_nonoverlapping(text.as_ptr(), output, n);
                output.add(n).write(0);
            }
        }
        text.len() as u64 + 1
    })
}
