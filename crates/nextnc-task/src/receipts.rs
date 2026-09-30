//! Task dispatch accounting, distinct from both read-ahead and physical completion.
//!
//! A source command can expand into multiple task messages. Reserve the entire
//! expansion before queueing it, begin each issue once, then record the actual
//! return at the recipient boundary. A lost/unknown result closes admission;
//! resending an uncertain motion is never a recovery mechanism.
use crate::lifecycle::{Binding, Error, Result};
use std::collections::VecDeque;

/// The boundary which must supply the result. None means physical completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipient {
    /// A task-owned state update applied successfully by the task thread.
    Task,
    /// The existing LinuxCNC I/O or process-command interface accepted the request.
    Io,
    /// The guarded motmod interface acknowledged the actual motion command.
    Motion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub binding: Binding,
    /// Original prepared-command index; every expanded piece keeps this identity.
    pub command: usize,
    pub piece: usize,
    pub recipient: Recipient,
    pub serial: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    Rejected,
    /// A timeout/disconnect cannot be interpreted as either no motion or success.
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub ticket: Ticket,
    pub outcome: Outcome,
    pub task_tick: u64,
    /// End-exclusive prepared-command prefix, after all its pieces are accepted.
    pub admitted: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts {
    pub queued_commands: usize,
    pub admitted_commands: usize,
    pub queued_pieces: u64,
    pub accepted_pieces: u64,
    pub pending_pieces: usize,
}

struct Piece {
    ticket: Ticket,
    last: bool,
}

/// Bounded task-message ledger. This object has no planner, HAL, or filesystem
/// access. Host integration must close Owner admission and stop on a fatal result.
pub struct Ledger {
    binding: Binding,
    capacity: usize,
    next_command: usize,
    admitted: usize,
    queued_pieces: u64,
    accepted_pieces: u64,
    pending: VecDeque<Piece>,
    in_flight: Option<(Ticket, u64)>,
    last_receipt: Option<Receipt>,
    last_tick: u64,
    closed: bool,
}

impl Ledger {
    pub fn new(binding: Binding, capacity: usize) -> Result<Self> {
        if capacity == 0
            || capacity > 4096
            || binding.generation.owner == 0
            || binding.generation.selection == 0
            || binding.state_epoch == 0
            || binding.state_sha256 == [0; 32]
            || binding.artifact_sha256 == [0; 32]
        {
            return Err(Error::Identity);
        }
        Ok(Self {
            binding,
            capacity,
            next_command: 0,
            admitted: 0,
            queued_pieces: 0,
            accepted_pieces: 0,
            pending: VecDeque::new(),
            in_flight: None,
            last_receipt: None,
            last_tick: 0,
            closed: false,
        })
    }

    pub fn counts(&self) -> Counts {
        Counts {
            queued_commands: self.next_command,
            admitted_commands: self.admitted,
            queued_pieces: self.queued_pieces,
            accepted_pieces: self.accepted_pieces,
            pending_pieces: self.pending.len(),
        }
    }
    pub fn closed(&self) -> bool {
        self.closed
    }
    pub fn last_receipt(&self) -> Option<Receipt> {
        self.last_receipt
    }
    pub fn available(&self) -> usize {
        if self.closed {
            0
        } else {
            self.capacity - self.pending.len()
        }
    }

    /// The next reserved message, before the host checks its task prerequisites.
    /// Looking does not authorize issue; `begin_issue` must still succeed directly
    /// before calling the recipient. An in-flight message cannot be looked up as
    /// a new dispatch after its outcome became uncertain.
    pub fn next_ticket(&self) -> Option<Ticket> {
        if self.closed || self.in_flight.is_some() {
            None
        } else {
            self.pending.front().map(|p| p.ticket)
        }
    }

    /// Atomic reservation: caller has already constructed and checked every
    /// message. Do not append half an expansion to the host queue on failure.
    /// No-op state changes still require an explicit task-owned application result.
    pub fn enqueue(
        &mut self,
        binding: Binding,
        command: usize,
        pieces: &[Recipient],
    ) -> Result<std::ops::Range<u64>> {
        if binding != self.binding {
            return Err(Error::Stale);
        }
        if self.closed {
            return Err(Error::State);
        }
        if command != self.next_command || pieces.is_empty() {
            return Err(Error::Sequence);
        }
        if pieces.len() > self.available() {
            return Err(Error::Busy);
        }
        let end = self
            .queued_pieces
            .checked_add(pieces.len() as u64)
            .ok_or(Error::Exhausted)?;
        let next = self.next_command.checked_add(1).ok_or(Error::Exhausted)?;
        // Exclusive range of serials; zero is reserved. Check the upper bound
        // before changing anything, even on the terminal counter value.
        let serials = self.queued_pieces.checked_add(1).ok_or(Error::Exhausted)?
            ..end.checked_add(1).ok_or(Error::Exhausted)?;
        for (piece, recipient) in pieces.iter().enumerate() {
            self.pending.push_back(Piece {
                ticket: Ticket {
                    binding,
                    command,
                    piece,
                    recipient: *recipient,
                    serial: serials.start + piece as u64,
                },
                last: piece + 1 == pieces.len(),
            });
        }
        self.next_command = next;
        self.queued_pieces = end;
        Ok(serials)
    }

    /// Call immediately before dispatching the first pending host message. Once
    /// returned, this ticket cannot be issued again, even if its result is lost.
    pub fn begin_issue(&mut self, task_tick: u64) -> Result<Option<Ticket>> {
        if self.closed {
            return Err(Error::State);
        }
        if self.in_flight.is_some() {
            return Err(Error::Busy);
        }
        if task_tick == 0 || task_tick < self.last_tick {
            return Err(Error::Sequence);
        }
        let Some(piece) = self.pending.front() else {
            return Ok(None);
        };
        self.in_flight = Some((piece.ticket, task_tick));
        self.last_tick = task_tick;
        Ok(Some(piece.ticket))
    }

    /// A receipt retry only repeats the last result; it never invokes the host.
    /// Unknown/rejected outcomes permanently close this ledger. Keep their
    /// diagnostic identity while Owner separately revokes execution authority.
    pub fn acknowledge(
        &mut self,
        ticket: Ticket,
        outcome: Outcome,
        task_tick: u64,
    ) -> Result<Receipt> {
        if ticket.binding != self.binding {
            return Err(Error::Stale);
        }
        if let Some(previous) = self.last_receipt {
            if previous.ticket == ticket {
                return if previous.outcome == outcome && previous.task_tick == task_tick {
                    Ok(previous)
                } else {
                    Err(Error::Sequence)
                };
            }
        }
        if self.closed {
            return Err(Error::State);
        }
        let (expected, issued_tick) = self.in_flight.ok_or(Error::Sequence)?;
        if expected != ticket || task_tick < issued_tick || task_tick < self.last_tick {
            return Err(Error::Sequence);
        }
        self.last_tick = task_tick;
        if outcome == Outcome::Accepted {
            let piece = self.pending.pop_front().ok_or(Error::Sequence)?;
            self.accepted_pieces += 1;
            if piece.last {
                self.admitted = ticket.command + 1;
            }
        } else {
            self.closed = true;
        }
        let receipt = Receipt {
            ticket,
            outcome,
            task_tick,
            admitted: self.admitted,
        };
        self.last_receipt = Some(receipt);
        self.in_flight = None;
        Ok(receipt)
    }

    /// Abort/disconnect invalidates pending work before host cleanup. There is no
    /// reopen operation; recovery must create a new selection and a new ledger.
    pub fn close(&mut self) {
        self.closed = true;
    }

    /// This checks only message accounting and observation ordering, never actual
    /// motion/I/O/shaper completion. Combine it with fresh pinned-host status.
    pub fn observed_after_dispatch(&self, task_tick: u64) -> bool {
        !self.closed
            && self.pending.is_empty()
            && self.in_flight.is_none()
            && self.last_receipt.is_some()
            && task_tick > self.last_tick
    }

    /// At a proven procedure drain, keep the same replay and admitted prefix but
    /// change live binding. Owner must verify the complete host drain first.
    pub fn rebind(&mut self, previous: Binding, next: Binding) -> Result<()> {
        if previous != self.binding {
            return Err(Error::Stale);
        }
        if self.closed || !self.pending.is_empty() || self.in_flight.is_some() {
            return Err(Error::NotDrained);
        }
        if next.generation != previous.generation
            || next.artifact_sha256 != previous.artifact_sha256
            || next.state_epoch <= previous.state_epoch
            || next.state_sha256 == [0; 32]
        {
            return Err(Error::Identity);
        }
        self.binding = next;
        self.last_receipt = None;
        Ok(())
    }
}
