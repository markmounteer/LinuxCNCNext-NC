//! Single task-owner lifecycle and contiguous-prefix receipts. Pure control
//! state: the host is responsible for evidence acquisition and physical actions.
use crate::steps::Layout;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation {
    pub owner: u64,
    pub selection: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub generation: Generation,
    /// Changes on coordinate/tool/configuration/recovery changes, not each pose.
    pub state_epoch: u64,
    /// Identifies the freshly captured live configuration and offsets.
    pub state_sha256: [u8; 32],
    pub artifact_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Empty,
    Loading,
    Selected,
    Armed,
    Running,
    Holding,
    Held,
    StepDrain,
    Draining,
    Reconciling,
    Complete,
    Aborting,
    Faulted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Busy,
    State,
    Stale,
    Identity,
    Sequence,
    Restart,
    NotReady,
    Exhausted,
    NotDrained,
}
pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native task lifecycle: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Continuous,
    Step,
}

/// All must be freshly read by the owning task; this is not a cached permit.
#[derive(Clone, Copy, Debug)]
pub struct Readiness {
    pub automatic: bool,
    pub enabled: bool,
    pub homed: bool,
    pub fault_free: bool,
    pub quiescent: bool,
    pub source_binding_current: bool,
    pub downstream_compatible: bool,
}
impl Readiness {
    fn ready(self) -> bool {
        self.resume_ready() && self.quiescent
    }
    fn resume_ready(self) -> bool {
        self.automatic
            && self.enabled
            && self.homed
            && self.fault_free
            && self.source_binding_current
            && self.downstream_compatible
    }
}

/// Completion evidence must cover task, I/O, planner and the delayed shaper tail.
#[derive(Clone, Copy, Debug)]
pub struct Drain {
    pub task_empty: bool,
    pub io_done: bool,
    pub motion_done: bool,
    pub in_position: bool,
    pub shaper_done: bool,
    pub fault_free: bool,
    pub observed_after_admission: bool,
}
impl Drain {
    fn complete(self) -> bool {
        self.task_empty
            && self.io_done
            && self.motion_done
            && self.in_position
            && self.shaper_done
            && self.fault_free
            && self.observed_after_admission
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub binding: Binding,
    pub serial: u64,
    pub commands: Range<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prefixes {
    /// End-exclusive counts. Accepted means task read-ahead, not motion admission.
    pub accepted: usize,
    pub admitted: usize,
    pub completed: usize,
    pub executing: Option<usize>,
}

/// Own one instance per task process; never restore this state from a bundle.
pub struct Owner {
    epoch: u64,
    selection: u64,
    phase: Phase,
    artifact: Option<[u8; 32]>,
    binding: Option<Binding>,
    layout: Option<Layout>,
    prefixes: Prefixes,
    offer: Option<Offer>,
    last_offer: Option<Offer>,
    serial: u64,
    capacity: usize,
    step_end: Option<usize>,
    resume_phase: Phase,
    boundary_proposal: Option<usize>,
}

impl Owner {
    pub fn new(epoch: u64, capacity: usize) -> Result<Self> {
        if epoch == 0 || capacity == 0 || capacity > 4096 {
            return Err(Error::Identity);
        }
        Ok(Self {
            epoch,
            selection: 0,
            phase: Phase::Empty,
            artifact: None,
            binding: None,
            layout: None,
            prefixes: Prefixes::default(),
            offer: None,
            last_offer: None,
            serial: 0,
            capacity,
            step_end: None,
            resume_phase: Phase::Running,
            boundary_proposal: None,
        })
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn prefixes(&self) -> Prefixes {
        self.prefixes
    }
    pub fn generation(&self) -> Generation {
        Generation {
            owner: self.epoch,
            selection: self.selection,
        }
    }
    pub fn binding(&self) -> Option<Binding> {
        self.binding
    }
    pub fn step_boundary(&self) -> Option<usize> {
        self.step_end
    }

    fn check_generation(&self, generation: Generation) -> Result<()> {
        if generation != self.generation() || generation.selection == 0 {
            Err(Error::Stale)
        } else {
            Ok(())
        }
    }
    fn check_binding(&self, binding: Binding) -> Result<()> {
        self.check_generation(binding.generation)?;
        if self.binding != Some(binding) {
            Err(Error::Stale)
        } else {
            Ok(())
        }
    }
    fn clear(&mut self) {
        self.artifact = None;
        self.binding = None;
        self.layout = None;
        self.offer = None;
        self.last_offer = None;
        self.prefixes = Prefixes::default();
        self.step_end = None;
        self.boundary_proposal = None;
    }

    /// Revoke the previous candidate BEFORE fallible/asynchronous file work.
    pub fn begin_select(&mut self) -> Result<Generation> {
        if !matches!(
            self.phase,
            Phase::Empty | Phase::Loading | Phase::Selected | Phase::Armed | Phase::Complete
        ) {
            return Err(Error::Busy);
        }
        self.clear();
        self.phase = Phase::Empty;
        self.selection = self.selection.checked_add(1).ok_or(Error::Exhausted)?;
        self.phase = Phase::Loading;
        Ok(self.generation())
    }
    /// Only a fully audited artifact may supply this layout/hash. No live binding
    /// is created here. The host's bundle loader is a separate checked boundary.
    pub fn selected(
        &mut self,
        generation: Generation,
        artifact: [u8; 32],
        layout: Layout,
    ) -> Result<()> {
        self.check_generation(generation)?;
        if self.phase != Phase::Loading {
            return Err(Error::State);
        }
        if artifact == [0; 32] || layout.commands() == 0 {
            return Err(Error::Identity);
        }
        self.artifact = Some(artifact);
        self.layout = Some(layout);
        self.phase = Phase::Selected;
        Ok(())
    }
    pub fn selection_failed(&mut self, generation: Generation) -> Result<()> {
        self.check_generation(generation)?;
        if !matches!(self.phase, Phase::Loading | Phase::Selected | Phase::Armed) {
            return Err(Error::State);
        }
        self.clear();
        self.phase = Phase::Empty;
        Ok(())
    }
    pub fn arm(&mut self, binding: Binding, ready: Readiness) -> Result<()> {
        self.check_generation(binding.generation)?;
        if self.phase != Phase::Selected {
            return Err(Error::State);
        }
        if !ready.ready() {
            return Err(Error::NotReady);
        }
        if binding.state_epoch == 0
            || binding.state_sha256 == [0; 32]
            || Some(binding.artifact_sha256) != self.artifact
        {
            return Err(Error::Identity);
        }
        self.binding = Some(binding);
        self.phase = Phase::Armed;
        Ok(())
    }
    pub fn start(
        &mut self,
        binding: Binding,
        ready: Readiness,
        mode: Start,
        restart: usize,
    ) -> Result<()> {
        self.check_binding(binding)?;
        if self.phase != Phase::Armed {
            return Err(Error::State);
        }
        if restart != 0 {
            return Err(Error::Restart);
        }
        if !ready.ready() {
            return Err(Error::NotReady);
        }
        self.step_end = match mode {
            Start::Continuous => None,
            Start::Step => Some(
                self.layout
                    .as_ref()
                    .and_then(|l| l.group_for(0))
                    .ok_or(Error::State)?
                    .commands
                    .end,
            ),
        };
        self.phase = Phase::Running;
        Ok(())
    }

    /// Retry returns the same immutable serial and remaining prefix. Capacity
    /// accounts for all task-accepted work not yet admitted, including state.
    pub fn offer(&mut self, maximum: usize) -> Result<Option<Offer>> {
        if self.phase != Phase::Running || maximum == 0 {
            return Ok(None);
        }
        let binding = self.binding.ok_or(Error::State)?;
        if let Some(offer) = &self.offer {
            return Ok(Some(Offer {
                commands: self.prefixes.accepted..offer.commands.end,
                ..offer.clone()
            }));
        }
        let layout = self.layout.as_ref().ok_or(Error::State)?;
        let mut end = layout.commands().min(self.step_end.unwrap_or(usize::MAX));
        // Tools/dwell/fences are read-ahead boundaries. The host owns the real
        // procedure and fresh result/rebinding; no suffix is offered past it.
        if let Some(g) = layout.next_barrier(self.prefixes.completed) {
            if self.prefixes.accepted >= g.end {
                return Ok(None);
            }
            end = end.min(g.end);
            if self.prefixes.accepted < g.start {
                end = end.min(g.start);
            } else if self.prefixes.completed < g.start {
                return Ok(None);
            }
        }
        if let Some(point) = layout.next_drain(self.prefixes.completed) {
            if self.prefixes.accepted >= point {
                return Ok(None);
            }
            end = end.min(point);
        }
        let available = self
            .capacity
            .saturating_sub(self.prefixes.accepted - self.prefixes.admitted);
        end = end.min(
            self.prefixes
                .accepted
                .saturating_add(maximum.min(available)),
        );
        if end <= self.prefixes.accepted {
            return Ok(None);
        }
        self.serial = self.serial.checked_add(1).ok_or(Error::Exhausted)?;
        let offer = Offer {
            binding,
            serial: self.serial,
            commands: self.prefixes.accepted..end,
        };
        self.offer = Some(offer.clone());
        Ok(Some(offer))
    }
    /// `end` is an absolute contiguous prefix, never a count to add twice.
    pub fn accept(&mut self, offer: &Offer, end: usize) -> Result<()> {
        self.check_binding(offer.binding)?;
        // A lost receipt may be retried after the command reached a drain or
        // hold boundary. Repeating the last accepted prefix cannot admit more.
        if let Some(last) = &self.last_offer {
            if last.serial == offer.serial
                && last.binding == offer.binding
                && offer.commands.end == last.commands.end
                && offer.commands.start >= last.commands.start
                && offer.commands.start < last.commands.end
                && end >= offer.commands.start
                && end <= last.commands.end
            {
                return Ok(());
            }
        }
        if self.phase != Phase::Running {
            return Err(Error::State);
        }
        let current = self.offer.as_ref().ok_or(Error::Sequence)?;
        if offer.serial != current.serial
            || offer.commands.end != current.commands.end
            || offer.commands.start < current.commands.start
            || offer.commands.start > self.prefixes.accepted
            || end < current.commands.start
            || end > current.commands.end
        {
            return Err(Error::Sequence);
        }
        if end > self.prefixes.accepted {
            self.prefixes.accepted = end;
        }
        if end == current.commands.end {
            self.last_offer = self.offer.clone();
            self.offer = None;
        }
        Ok(())
    }
    pub fn admitted(&mut self, binding: Binding, end: usize) -> Result<()> {
        self.check_binding(binding)?;
        if !matches!(
            self.phase,
            Phase::Running | Phase::Holding | Phase::Held | Phase::StepDrain | Phase::Draining
        ) {
            return Err(Error::State);
        }
        if end < self.prefixes.admitted || end > self.prefixes.accepted {
            return Err(Error::Sequence);
        }
        self.prefixes.admitted = end;
        let total = self.layout.as_ref().ok_or(Error::State)?.commands();
        let next = if end == total {
            Some(Phase::Draining)
        } else if self.step_end == Some(end) {
            Some(Phase::StepDrain)
        } else {
            None
        };
        if let Some(next) = next {
            if matches!(self.phase, Phase::Holding | Phase::Held) {
                self.resume_phase = next;
            } else {
                self.phase = next;
            }
        }
        Ok(())
    }
    pub fn executing(&mut self, binding: Binding, command: usize) -> Result<()> {
        self.check_binding(binding)?;
        if command >= self.prefixes.admitted || command < self.prefixes.completed {
            return Err(Error::Sequence);
        }
        if self.prefixes.executing.is_some_and(|old| command < old) {
            return Err(Error::Sequence);
        }
        self.prefixes.executing = Some(command);
        Ok(())
    }
    pub fn drained(&mut self, binding: Binding, evidence: Drain) -> Result<()> {
        self.check_binding(binding)?;
        if !matches!(
            self.phase,
            Phase::Running | Phase::StepDrain | Phase::Draining
        ) {
            return Err(Error::State);
        }
        if !evidence.complete() || self.prefixes.accepted != self.prefixes.admitted {
            return Err(Error::NotDrained);
        }
        self.prefixes.completed = self.prefixes.admitted;
        self.prefixes.executing = None;
        if self.phase == Phase::Draining {
            self.phase = Phase::Reconciling;
        } else if self.phase == Phase::StepDrain {
            self.phase = Phase::Held;
            self.resume_phase = Phase::Running;
        }
        Ok(())
    }
    pub fn hold(&mut self) -> Result<()> {
        if matches!(self.phase, Phase::Holding | Phase::Held) {
            return Ok(());
        }
        if !matches!(
            self.phase,
            Phase::Running | Phase::StepDrain | Phase::Draining
        ) {
            return Err(Error::State);
        }
        self.resume_phase = self.phase;
        self.phase = Phase::Holding;
        // Unaccepted ranges can never escape the hold. Already accepted work
        // remains accounted for, so a step cannot promise an earlier boundary.
        self.offer = None;
        Ok(())
    }
    pub fn held(&mut self, binding: Binding, host_hold_acknowledged: bool) -> Result<()> {
        self.check_binding(binding)?;
        if self.phase != Phase::Holding {
            return Err(Error::State);
        }
        if !host_hold_acknowledged {
            return Err(Error::NotReady);
        }
        self.phase = Phase::Held;
        Ok(())
    }
    pub fn propose_step(&mut self, binding: Binding) -> Result<usize> {
        self.check_binding(binding)?;
        if self.phase != Phase::Held {
            return Err(Error::State);
        }
        let layout = self.layout.as_ref().ok_or(Error::State)?;
        let index = if self.prefixes.accepted > self.prefixes.completed {
            self.prefixes.accepted - 1
        } else {
            self.prefixes.completed
        };
        let end = layout.group_for(index).ok_or(Error::State)?.commands.end;
        self.boundary_proposal = Some(end);
        Ok(end)
    }
    pub fn resume(
        &mut self,
        binding: Binding,
        ready: Readiness,
        acknowledged_step_end: Option<usize>,
    ) -> Result<()> {
        self.check_binding(binding)?;
        if self.phase != Phase::Held {
            return Err(Error::State);
        }
        if !ready.resume_ready() {
            return Err(Error::NotReady);
        }
        if let Some(end) = acknowledged_step_end {
            if self.boundary_proposal != Some(end) {
                return Err(Error::Sequence);
            }
            self.step_end = Some(end);
            self.phase = if self.prefixes.admitted == end {
                Phase::StepDrain
            } else {
                Phase::Running
            };
        } else {
            self.step_end = None;
            self.phase = if self.resume_phase == Phase::Draining {
                Phase::Draining
            } else {
                Phase::Running
            };
        }
        self.boundary_proposal = None;
        Ok(())
    }
    /// Close admission BEFORE host abort/cleanup. Late offers and worker results
    /// lose authority immediately. Recovery cannot restore the old cursor.
    pub fn abort(&mut self) {
        self.clear();
        self.phase = Phase::Aborting;
    }
    pub fn fault(&mut self) {
        self.clear();
        self.phase = Phase::Faulted;
    }
    /// Connection loss uses the same controller-owned abort policy.
    pub fn disconnected(&mut self) {
        self.abort();
    }
    /// A procedure may change coordinates/tool state only at a fully drained
    /// boundary. Rebinding retains the job/prefix but invalidates old receipts.
    pub fn rebind(&mut self, previous: Binding, next: Binding, evidence: Drain) -> Result<()> {
        self.check_binding(previous)?;
        if self.phase != Phase::Running || self.offer.is_some() {
            return Err(Error::State);
        }
        if !evidence.complete()
            || self.prefixes.completed != self.prefixes.accepted
            || self.prefixes.admitted != self.prefixes.accepted
        {
            return Err(Error::NotDrained);
        }
        if previous.generation != next.generation
            || previous.artifact_sha256 != next.artifact_sha256
            || next.state_epoch <= previous.state_epoch
            || next.state_sha256 == [0; 32]
        {
            return Err(Error::Identity);
        }
        self.binding = Some(next);
        self.last_offer = None;
        Ok(())
    }
    pub fn reconciled(&mut self, evidence: Drain, state_agrees: bool) -> Result<()> {
        if !matches!(
            self.phase,
            Phase::Reconciling | Phase::Aborting | Phase::Faulted
        ) {
            return Err(Error::State);
        }
        if !evidence.complete() || !state_agrees {
            return Err(Error::NotDrained);
        }
        let completed = self.phase == Phase::Reconciling;
        self.binding = None;
        self.offer = None;
        self.phase = if completed {
            Phase::Complete
        } else {
            Phase::Empty
        };
        Ok(())
    }
    pub fn allows_mdi(&self) -> bool {
        matches!(self.phase, Phase::Empty | Phase::Complete)
    }
}
