//! Task-thread execution authority. Preparation handles alone never authorize
//! dispatch. This ABI joins the independently audited candidate, Owner and Ledger;
//! only the pinned host can supply actual readiness, recipient results and drains.
//! No function here writes motion, executes G-code, or fabricates completion.
use crate::{address, boundary, registry, wire, Candidate, Result};
use nextnc_native::bundle;
use nextnc_task::{
    lifecycle::{Binding, Drain, Generation, Owner, Phase, Readiness, Start},
    receipts::{Ledger, Outcome, Recipient, Ticket},
};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, ThreadId};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fingerprint {
    pub abi: u32,
    pub bytes: u32,
    /// Configuration/offset/tool/dynamics identity, excluding commanded pose.
    pub configuration: [u8; 32],
    /// The same identity plus all nine commanded pose components.
    pub initial: [u8; 32],
}

fn hash(bytes: &[u8]) -> Result<[u8; 32]> {
    unhex(&bundle::digest(bytes))
}
fn unhex(text: &str) -> Result<[u8; 32]> {
    let mut out = [0; 32];
    if text.len() != 64 || !text.is_ascii() {
        return Err("invalid SHA-256 identity".into());
    }
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
            .map_err(|_| "invalid SHA-256 identity")?;
    }
    Ok(out)
}

/// Padding and input tool-table order are not identities. Every dimension and
/// supported capability is; signed zero is deliberately exact, not a tolerance.
pub(crate) fn fingerprint(s: &wire::Snapshot, tools: &[wire::Tool]) -> Result<Fingerprint> {
    s.decode(tools)?;
    let mut bytes = b"nextnc-live-snapshot-v1\0".to_vec();
    for n in [
        s.machine,
        s.axis_mask,
        s.work_offset,
        s.shaping,
        s.capabilities,
    ] {
        bytes.extend_from_slice(&n.to_le_bytes());
    }
    let floats = |bytes: &mut Vec<u8>, values: &[f64]| -> Result<()> {
        for v in values {
            if !v.is_finite() {
                return Err("nonfinite live fingerprint value".into());
            }
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        Ok(())
    };
    for work in &s.work {
        floats(&mut bytes, work)?;
    }
    for values in [
        s.rotation.as_slice(),
        s.temporary.as_slice(),
        s.tool_offset.as_slice(),
        s.minimum.as_slice(),
        s.maximum.as_slice(),
        s.velocity.as_slice(),
        s.acceleration.as_slice(),
        s.jerk.as_slice(),
        std::slice::from_ref(&s.maximum_rpm),
    ] {
        floats(&mut bytes, values)?;
    }
    let mut sorted: Vec<_> = tools.iter().collect();
    sorted.sort_by_key(|t| t.number);
    bytes.extend_from_slice(&(sorted.len() as u64).to_le_bytes());
    for t in sorted {
        bytes.extend_from_slice(&t.number.to_le_bytes());
        floats(&mut bytes, &t.offset)?;
    }
    let configuration = hash(&bytes)?;
    floats(&mut bytes, &s.pose)?;
    Ok(Fingerprint {
        abi: wire::ABI,
        bytes: std::mem::size_of::<Fingerprint>() as u32,
        configuration,
        initial: hash(&bytes)?,
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Dispatch {
    pub abi: u32,
    pub bytes: u32,
    pub selection: u64,
    /// Zero means no message. A nonzero serial is only a reservation: `issue`
    /// must succeed immediately before calling the checked host recipient.
    pub serial: u64,
    pub recipient: u32,
    pub reserved: u32,
    pub message: wire::Message,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Status {
    pub abi: u32,
    pub bytes: u32,
    pub phase: u32,
    pub allows_mdi: u32,
    pub selection: u64,
    pub candidate: u64,
    pub accepted: u64,
    pub admitted: u64,
    pub completed: u64,
    pub queued_pieces: u64,
    pub accepted_pieces: u64,
    pub pending_pieces: u64,
    pub proposed_step_end: u64,
}

struct Runtime {
    thread: ThreadId,
    owner: Owner,
    capacity: usize,
    candidate: Option<(u64, Arc<Candidate>)>,
    ledger: Option<Ledger>,
    issued: Option<Ticket>,
    clock: u64,
    stop_tick: u64,
    proposed: Option<usize>,
}
#[derive(Default)]
struct Owners {
    next: u64,
    live: Option<(u64, Runtime)>,
}
static OWNERS: OnceLock<Mutex<Owners>> = OnceLock::new();
fn owners() -> Result<std::sync::MutexGuard<'static, Owners>> {
    OWNERS
        .get_or_init(|| Mutex::new(Owners::default()))
        .lock()
        .map_err(|_| "native owner mutex poisoned; stop and restart task".into())
}
fn with<T>(handle: u64, f: impl FnOnce(&mut Runtime) -> Result<T>) -> Result<T> {
    let mut owners = owners()?;
    let (id, runtime) = owners.live.as_mut().ok_or("native owner does not exist")?;
    if handle == 0 || *id != handle {
        return Err("stale native owner handle".into());
    }
    if runtime.thread != thread::current().id() {
        return Err("native lifecycle must stay on its owning task thread".into());
    }
    f(runtime)
}
fn readiness(flags: u32) -> Result<Readiness> {
    if flags & !127 != 0 {
        return Err("unknown readiness flags".into());
    }
    Ok(Readiness {
        automatic: flags & 1 != 0,
        enabled: flags & 2 != 0,
        homed: flags & 4 != 0,
        fault_free: flags & 8 != 0,
        quiescent: flags & 16 != 0,
        source_binding_current: flags & 32 != 0,
        downstream_compatible: flags & 64 != 0,
    })
}
fn drain(flags: u32, observed: bool) -> Result<Drain> {
    if flags & !63 != 0 {
        return Err("unknown drain flags".into());
    }
    Ok(Drain {
        task_empty: flags & 1 != 0,
        io_done: flags & 2 != 0,
        motion_done: flags & 4 != 0,
        in_position: flags & 8 != 0,
        shaper_done: flags & 16 != 0,
        fault_free: flags & 32 != 0,
        observed_after_admission: observed,
    })
}
fn phase(p: Phase) -> u32 {
    match p {
        Phase::Empty => 0,
        Phase::Loading => 1,
        Phase::Selected => 2,
        Phase::Armed => 3,
        Phase::Running => 4,
        Phase::Holding => 5,
        Phase::Held => 6,
        Phase::StepDrain => 7,
        Phase::Draining => 8,
        Phase::Reconciling => 9,
        Phase::Complete => 10,
        Phase::Aborting => 11,
        Phase::Faulted => 12,
    }
}
fn recipient(kind: u32) -> Result<Recipient> {
    match kind {
        1 | 2 | 4 | 12 | 14 | 15 | 17 => Ok(Recipient::Motion),
        16 | 18 => Ok(Recipient::Io),
        3 | 10 | 11 | 13 | 19 | 20 | 21 => Ok(Recipient::Task),
        _ => Err("unsupported task recipient".into()),
    }
}
fn recipient_number(r: Recipient) -> u32 {
    match r {
        Recipient::Task => 1,
        Recipient::Io => 2,
        Recipient::Motion => 3,
    }
}
impl Runtime {
    fn tick(&mut self, tick: u64) -> Result<()> {
        if tick == 0 || tick < self.clock {
            return Err("stale task heartbeat".into());
        }
        self.clock = tick;
        Ok(())
    }
    fn binding(&self) -> Result<Binding> {
        self.owner
            .binding()
            .ok_or_else(|| "native owner is not bound".into())
    }
    fn stop(&mut self, fault: bool, disconnected: bool, tick: u64) {
        // Even a broken heartbeat must not prevent revocation or host stopping.
        self.stop_tick = self.clock.max(tick);
        if let Some(ledger) = &mut self.ledger {
            ledger.close();
        }
        self.issued = None;
        self.proposed = None;
        if fault {
            self.owner.fault();
        } else if disconnected {
            self.owner.disconnected();
        } else {
            self.owner.abort();
        }
    }
    fn next(&mut self) -> Result<Dispatch> {
        let mut out = Dispatch {
            abi: wire::ABI,
            bytes: std::mem::size_of::<Dispatch>() as u32,
            ..Dispatch::default()
        };
        if !matches!(
            self.owner.phase(),
            Phase::Running | Phase::StepDrain | Phase::Draining
        ) {
            return Ok(out);
        }
        let candidate = &self.candidate.as_ref().ok_or("no native candidate")?.1;
        let ledger = self.ledger.as_mut().ok_or("no native dispatch ledger")?;
        if self.issued.is_some() {
            return Err("recipient outcome still pending".into());
        }
        if ledger.counts().pending_pieces == 0 {
            if let Some(offer) = self.owner.offer(1).map_err(|e| e.to_string())? {
                let command = offer.commands.start;
                let range = candidate
                    .lowered
                    .commands()
                    .get(command)
                    .ok_or("bad native range")?;
                let recipients = range
                    .clone()
                    .map(|p| {
                        let message =
                            wire::encode(&candidate.lowered.pieces()[p], range.len(), false)?;
                        recipient(message.kind)
                    })
                    .collect::<Result<Vec<_>>>()?;
                // Reserve every expansion piece, then acknowledge read-ahead.
                ledger
                    .enqueue(offer.binding, command, &recipients)
                    .map_err(|e| e.to_string())?;
                if let Err(e) = self.owner.accept(&offer, command + 1) {
                    ledger.close();
                    self.owner.fault();
                    return Err(e.to_string());
                }
            }
        }
        if let Some(ticket) = ledger.next_ticket() {
            let range = &candidate.lowered.commands()[ticket.command];
            out.selection = ticket.binding.generation.selection;
            out.serial = ticket.serial;
            out.recipient = recipient_number(ticket.recipient);
            out.message = wire::encode(
                &candidate.lowered.pieces()[range.start + ticket.piece],
                range.len(),
                candidate
                    .lowered
                    .drains_before()
                    .binary_search(&ticket.command)
                    .is_ok(),
            )?;
        }
        Ok(out)
    }
}

/// # Safety
/// Inputs provide valid immutable extents. Output is writable and disjoint.
#[no_mangle]
pub unsafe extern "C" fn nextnc_task_fingerprint(
    snapshot: *const wire::Snapshot,
    tools: *const wire::Tool,
    count: u64,
    output: *mut Fingerprint,
    output_size: u64,
) -> i32 {
    boundary(|| {
        if output_size != std::mem::size_of::<Fingerprint>() as u64 {
            return Err("invalid fingerprint output size".into());
        }
        address(output)?;
        // SAFETY: caller supplies the checked output extent.
        unsafe {
            output.write(Fingerprint::default());
        }
        address(snapshot)?;
        // SAFETY: input contract includes at least the common eight-byte header.
        let header = unsafe { snapshot.cast::<[u32; 2]>().read() };
        if header != [wire::ABI, std::mem::size_of::<wire::Snapshot>() as u32] {
            return Err("unsupported snapshot ABI/size".into());
        }
        if count > wire::MAX_TOOLS as u64 {
            return Err("tool table too large".into());
        }
        if count > 0 {
            address(tools)?;
        }
        // SAFETY: all input extents are supplied by caller and bounded above.
        let result = unsafe {
            let table = if count == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(tools, count as usize)
            };
            fingerprint(&snapshot.read(), table)?
        };
        // SAFETY: output remains writable for this synchronous call.
        unsafe {
            output.write(result);
        }
        Ok(())
    })
}

/// Create the single owner on the task thread. Handles/epochs never repeat.
/// # Safety
/// Output must point to a writable aligned u64.
#[no_mangle]
pub unsafe extern "C" fn nextnc_owner_create(capacity: u32, output: *mut u64) -> i32 {
    boundary(|| {
        address(output)?;
        // SAFETY: caller supplies a writable u64.
        unsafe {
            output.write(0);
        }
        if !(2..=4096).contains(&capacity) {
            return Err("invalid message capacity".into());
        }
        let mut r = owners()?;
        if r.live.is_some() {
            return Err("a native task owner already exists".into());
        }
        let handle = r
            .next
            .checked_add(1)
            .ok_or("owner handle space exhausted")?;
        let owner = Owner::new(handle, capacity as usize).map_err(|e| e.to_string())?;
        r.next = handle;
        r.live = Some((
            handle,
            Runtime {
                thread: thread::current().id(),
                owner,
                capacity: capacity as usize,
                candidate: None,
                ledger: None,
                issued: None,
                clock: 0,
                stop_tick: 0,
                proposed: None,
            },
        ));
        // SAFETY: output still satisfies its synchronous lifetime contract.
        unsafe {
            output.write(handle);
        }
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn nextnc_owner_destroy(handle: u64) -> i32 {
    boundary(|| {
        with(handle, |r| {
            if !r.owner.allows_mdi() {
                return Err("cannot destroy unreconciled native owner".into());
            }
            Ok(())
        })?;
        // Candidate's registry reference guarantees its large allocation is not
        // dropped on this task thread. Worker release follows detachment.
        owners()?.live = None;
        Ok(())
    })
}

/// Revoke selection before starting fallible worker/file work.
/// # Safety
/// Output must point to a writable aligned u64.
#[no_mangle]
pub unsafe extern "C" fn nextnc_owner_begin(handle: u64, output: *mut u64) -> i32 {
    boundary(|| {
        address(output)?;
        // SAFETY: output extent supplied by caller.
        unsafe {
            output.write(0);
        }
        let selection = with(handle, |r| {
            let generation = r.owner.begin_select().map_err(|e| e.to_string())?;
            r.ledger = None;
            r.candidate = None;
            r.issued = None;
            r.proposed = None;
            Ok(generation.selection)
        })?;
        // SAFETY: output remains valid during this call.
        unsafe {
            output.write(selection);
        }
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn nextnc_owner_attach(handle: u64, selection: u64, candidate: u64) -> i32 {
    boundary(|| {
        let c = registry()
            .candidates
            .get(&candidate)
            .cloned()
            .ok_or("stale candidate handle")?;
        with(handle, |r| {
            r.owner
                .selected_shared(
                    Generation {
                        owner: handle,
                        selection,
                    },
                    unhex(c.artifact.sha256())?,
                    Arc::clone(&c.layout),
                )
                .map_err(|e| e.to_string())?;
            r.candidate = Some((candidate, c));
            Ok(())
        })
    })
}
#[no_mangle]
pub extern "C" fn nextnc_owner_failed(handle: u64, selection: u64) -> i32 {
    boundary(|| {
        with(handle, |r| {
            r.owner
                .selection_failed(Generation {
                    owner: handle,
                    selection,
                })
                .map_err(|e| e.to_string())?;
            r.candidate = None;
            r.ledger = None;
            Ok(())
        })
    })
}

/// # Safety
/// Fresh fingerprint must be a valid immutable struct with its common header.
#[no_mangle]
pub unsafe extern "C" fn nextnc_owner_start(
    handle: u64,
    fresh: *const Fingerprint,
    epoch: u64,
    flags: u32,
    mode: u32,
    restart: u64,
) -> i32 {
    boundary(|| {
        address(fresh)?;
        // SAFETY: every version must supply the eight-byte common header.
        let header = unsafe { fresh.cast::<[u32; 2]>().read() };
        if header != [wire::ABI, std::mem::size_of::<Fingerprint>() as u32] {
            return Err("unsupported fingerprint ABI/size".into());
        }
        // SAFETY: full immutable struct extent is the caller contract.
        let fresh = unsafe { fresh.read() };
        let ready = readiness(flags)?;
        let mode = match mode {
            0 => Start::Continuous,
            1 => Start::Step,
            _ => return Err("invalid start mode".into()),
        };
        if restart != 0 {
            return Err("native restart index must be zero".into());
        }
        with(handle, |r| {
            let c = &r.candidate.as_ref().ok_or("no native selection")?.1;
            if c.fingerprint != fresh {
                return Err("live pose/configuration changed since binding".into());
            }
            let binding = Binding {
                generation: r.owner.generation(),
                state_epoch: epoch,
                state_sha256: fresh.configuration,
                artifact_sha256: unhex(c.artifact.sha256())?,
            };
            let ledger = Ledger::new(binding, r.capacity).map_err(|e| e.to_string())?;
            r.owner.arm(binding, ready).map_err(|e| e.to_string())?;
            r.owner
                .start(binding, ready, mode, 0)
                .map_err(|e| e.to_string())?;
            r.ledger = Some(ledger);
            Ok(())
        })
    })
}

/// Read/reserve the next message without issuing it. Repeat reads are identical.
/// # Safety
/// Output provides exactly the stated writable extent, without aliasing.
#[no_mangle]
pub unsafe extern "C" fn nextnc_owner_next(handle: u64, output: *mut Dispatch, size: u64) -> i32 {
    boundary(|| {
        if size != std::mem::size_of::<Dispatch>() as u64 {
            return Err("invalid dispatch size".into());
        }
        address(output)?;
        // SAFETY: checked output extent is supplied by caller.
        unsafe {
            output.write(Dispatch::default());
        }
        let result = with(handle, Runtime::next)?;
        // SAFETY: output is still valid and no Rust pointers are copied.
        unsafe {
            output.write(result);
        }
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn nextnc_owner_issue(handle: u64, selection: u64, serial: u64, tick: u64) -> i32 {
    boundary(|| {
        with(handle, |r| {
            r.tick(tick)?;
            if !matches!(
                r.owner.phase(),
                Phase::Running | Phase::StepDrain | Phase::Draining
            ) {
                return Err("native owner is not issuing".into());
            }
            let ledger = r.ledger.as_mut().ok_or("no ledger")?;
            let ticket = ledger.next_ticket().ok_or("no issuable message")?;
            if ticket.serial != serial || ticket.binding.generation.selection != selection {
                return Err("stale dispatch identity".into());
            }
            r.issued = ledger.begin_issue(tick).map_err(|e| e.to_string())?;
            Ok(())
        })
    })
}
#[no_mangle]
pub extern "C" fn nextnc_owner_result(
    handle: u64,
    selection: u64,
    serial: u64,
    outcome: u32,
    tick: u64,
) -> i32 {
    boundary(|| {
        with(handle, |r| {
            let outcome = match outcome {
                0 => Outcome::Accepted,
                1 => Outcome::Rejected,
                2 => Outcome::Unknown,
                _ => return Err("invalid recipient result".into()),
            };
            r.tick(tick)?;
            let ledger = r.ledger.as_mut().ok_or("no ledger")?;
            let ticket = r
                .issued
                .or_else(|| ledger.last_receipt().map(|p| p.ticket))
                .ok_or("no issued message")?;
            if ticket.serial != serial || ticket.binding.generation.selection != selection {
                return Err("stale dispatch result".into());
            }
            let receipt = ledger
                .acknowledge(ticket, outcome, tick)
                .map_err(|e| e.to_string())?;
            r.issued = None;
            if outcome != Outcome::Accepted {
                r.stop(true, false, tick);
                return Err(
                    "recipient refused or outcome unknown; native admission revoked".into(),
                );
            }
            r.owner
                .admitted(ticket.binding, receipt.admitted)
                .map_err(|e| e.to_string())
        })
    })
}

/// Control operations are described in the public C header. Stop operations
/// revoke authority even with a stale heartbeat; all other operations are ordered.
#[no_mangle]
pub extern "C" fn nextnc_owner_control(
    handle: u64,
    operation: u32,
    argument: u64,
    flags: u32,
    tick: u64,
) -> i32 {
    boundary(|| {
        with(handle, |r| {
            if (6..=8).contains(&operation) {
                r.stop(operation == 7, operation == 8, tick);
                return Ok(());
            }
            r.tick(tick)?;
            match operation {
                1 if argument == 0 && flags == 0 => r.owner.hold().map_err(|e| e.to_string()),
                2 if argument <= 1 && flags == 0 => r
                    .owner
                    .held(r.binding()?, argument == 1)
                    .map_err(|e| e.to_string()),
                3 => {
                    let end = if argument == 0 {
                        None
                    } else {
                        Some(usize::try_from(argument).map_err(|_| "step end overflow")?)
                    };
                    r.owner
                        .resume(r.binding()?, readiness(flags)?, end)
                        .map_err(|e| e.to_string())?;
                    r.proposed = None;
                    Ok(())
                }
                4 if argument == 0 && flags == 0 => {
                    r.proposed = Some(
                        r.owner
                            .propose_step(r.binding()?)
                            .map_err(|e| e.to_string())?,
                    );
                    Ok(())
                }
                5 if argument == 0 => {
                    let observed = r
                        .ledger
                        .as_ref()
                        .is_some_and(|l| l.observed_after_dispatch(tick));
                    r.owner
                        .drained(r.binding()?, drain(flags, observed)?)
                        .map_err(|e| e.to_string())
                }
                9 if argument <= 1 => {
                    let observed = tick > r.stop_tick
                        && r.ledger
                            .as_ref()
                            .is_none_or(|l| l.closed() || l.observed_after_dispatch(tick));
                    r.owner
                        .reconciled(drain(flags, observed)?, argument == 1)
                        .map_err(|e| e.to_string())
                }
                _ => Err("invalid owner control operation/arguments".into()),
            }
        })
    })
}

/// # Safety
/// Output must provide the exact writable struct extent and not alias inputs.
#[no_mangle]
pub unsafe extern "C" fn nextnc_owner_status(handle: u64, output: *mut Status, size: u64) -> i32 {
    boundary(|| {
        if size != std::mem::size_of::<Status>() as u64 {
            return Err("invalid status size".into());
        }
        address(output)?;
        // SAFETY: caller provides the checked writable extent.
        unsafe {
            output.write(Status::default());
        }
        let result = with(handle, |r| {
            let p = r.owner.prefixes();
            let counts = r.ledger.as_ref().map(Ledger::counts);
            Ok(Status {
                abi: wire::ABI,
                bytes: std::mem::size_of::<Status>() as u32,
                phase: phase(r.owner.phase()),
                allows_mdi: u32::from(r.owner.allows_mdi()),
                selection: r.owner.generation().selection,
                candidate: r.candidate.as_ref().map_or(0, |c| c.0),
                accepted: p.accepted as u64,
                admitted: p.admitted as u64,
                completed: p.completed as u64,
                queued_pieces: counts.map_or(0, |c| c.queued_pieces),
                accepted_pieces: counts.map_or(0, |c| c.accepted_pieces),
                pending_pieces: counts.map_or(0, |c| c.pending_pieces as u64),
                proposed_step_end: r.proposed.map_or(0, |v| v as u64),
            })
        })?;
        // SAFETY: output remains valid and contains only fixed-width values.
        unsafe {
            output.write(result);
        }
        Ok(())
    })
}
