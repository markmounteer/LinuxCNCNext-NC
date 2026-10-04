//! Native Next-NC command vocabulary, before a controller adapter exists.
//!
//! This is a Rust semantic contract, NOT a C ABI, wire encoding, queue writer,
//! geometry oracle, or authorization to move. Structural validation is only one
//! precondition: the future task adapter must resolve offsets, check geometry,
//! capabilities, limits, live state and continuity before publishing anything.
//! See the crate README and `docs/native-capability-matrix.md` in this workspace.
//!
//! All resolved lengths are millimetres; speeds are explicitly dimensioned.
//! No allocation, dependencies, hardware access or unsafe code.

#![no_std]
#![forbid(unsafe_code)]

/// Revision 2 preserves analytic helices, source tolerance and movement intent.
/// Revision 1 remains at the crate root for explicit compatibility checks.
pub mod v2;

/// Semantic contract revision. Does not version LinuxCNC's private ABI.
pub const CONTRACT_VERSION: u32 = 1;

/// Supported Cartesian machine profiles; neither implies rotary support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine {
    /// Three linear axes.
    MillXyz,
    /// Two linear axes; X is always radius, Y must be zero.
    LatheXz,
}

/// Absolute Cartesian position after work/tool transforms, before kinematics.
/// Unconfigured/auxiliary axes must be checked against the controller separately;
/// the absence of auxiliary fields never permits resetting those axes to zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointMm {
    /// X, radius on a lathe.
    pub x: f64,
    /// Y; zero on an XZ lathe.
    pub y: f64,
    /// Z.
    pub z: f64,
}

impl PointMm {
    fn check(self, machine: Machine) -> Result<(), ContractError> {
        if !self.x.is_finite() || !self.y.is_finite() || !self.z.is_finite() {
            return Err(ContractError::NonFinite);
        }
        if machine == Machine::LatheXz && self.y != 0.0 {
            return Err(ContractError::AxisMismatch);
        }
        Ok(())
    }
}

/// Right-handed plane bases: XY=(X,Y), XZ=(Z,X), YZ=(Y,Z).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plane {
    /// Normal +Z.
    Xy,
    /// Normal +Y.
    Xz,
    /// Normal +X.
    Yz,
}

impl Plane {
    fn normal_coordinate(self, p: PointMm) -> f64 {
        match self {
            Self::Xy => p.z,
            Self::Xz => p.y,
            Self::Yz => p.x,
        }
    }
}

/// Viewed from the positive plane normal towards the origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    /// Clockwise in that view.
    Clockwise,
    /// Counterclockwise in that view.
    Counterclockwise,
}

/// V1 has single-turn planar arcs; helices and multi-turn arcs require extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcExtent {
    /// Noncoincident endpoints; sweep strictly between zero and one turn.
    Partial,
    /// Exactly one complete turn; coincident endpoints are explicit.
    FullCircle,
}

/// Resolved geometry. Start is an expectation, never a request to teleport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geometry {
    /// Straight segment, including source zero-length segments.
    Line {
        /// Expected start in the bound coordinate state.
        start: PointMm,
        /// Absolute end.
        end: PointMm,
    },
    /// Analytic planar arc. Radius consistency/admissibility need the geometry oracle.
    Arc {
        /// Expected start.
        start: PointMm,
        /// Absolute end.
        end: PointMm,
        /// Absolute centre, in the same plane and coordinate state.
        center: PointMm,
        /// Right-handed plane.
        plane: Plane,
        /// Explicit direction.
        rotation: Rotation,
        /// Explicit sweep category.
        extent: ArcExtent,
    },
}

/// Feed intent; speed caps and trajectory timing remain controller-owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Feed {
    /// Coordinated traverse at controller rapid limits.
    Rapid,
    /// Linear path speed in millimetres/second, converted once from source units.
    PerSecond(f64),
    /// Millimetres/revolution, with runtime spindle feedback (never fixed RPM conversion).
    PerRevolution {
        /// Positive distance per revolution.
        mm_per_rev: f64,
        /// Selected spindle; v1 supports spindle zero only.
        spindle: u8,
    },
}

/// Exit policy, independent of whether another command currently fills the queue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Termination {
    /// Preserve the exact path; this is not synonymous with a stop at every vertex.
    ExactPath,
    /// Come to rest at this boundary.
    ExactStop,
    /// Permitted Cartesian departure from the source path, in millimetres.
    Blend {
        /// Positive, explicit budget; no implicit controller default.
        max_deviation_mm: f64,
    },
}

/// Entry condition which optimization must not erase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryGate {
    /// No additional spindle-at-speed condition.
    None,
    /// Preserve the existing shim's configured-spindle at-speed gate semantics.
    SpindlesAtSpeed,
}

/// A prepared motion request. No velocity profile or motor samples are embedded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Absolute analytic geometry.
    pub geometry: Geometry,
    /// Feed intent.
    pub feed: Feed,
    /// Exit geometry/stop policy.
    pub termination: Termination,
    /// Required entry synchronization.
    pub entry_gate: EntryGate,
}

/// Spindle demands in canonical physical units; stopping is its own command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spindle {
    /// Stop spindle zero.
    Stop,
    /// Constant rotational speed.
    Rpm {
        /// Positive revolutions/minute.
        rpm: f64,
        /// Direction relative to the configured spindle convention.
        clockwise: bool,
    },
    /// Lathe surface-speed demand; the controller owns the radius-dependent RPM.
    Css {
        /// Positive tangential surface speed in millimetres/second.
        surface_mm_per_second: f64,
        /// Positive RPM ceiling.
        maximum_rpm: f64,
        /// Direction relative to the configured spindle convention.
        clockwise: bool,
    },
}

/// Coolant events are task-coordinated, not synchronized arbitrary HAL writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coolant {
    /// Both outputs off.
    Off,
    /// Flood enabled, mist off.
    Flood,
    /// Mist enabled, flood off.
    Mist,
}

/// Ordered events. All non-motion events are conservative drain-and-stop barriers
/// in v1; controller completion is required before any following motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    /// Native geometry admission.
    Motion(Motion),
    /// Spindle zero command.
    Spindle(Spindle),
    /// Coolant event.
    Coolant(Coolant),
    /// Run the configured physical tool-change procedure; zero may unload.
    ChangeTool {
        /// Physical tool number, independently mapped from the source tool.
        tool: u32,
    },
    /// Select an independent tool-table offset; zero cancels it. Requires rebinding.
    ToolOffset {
        /// Tool-table offset number, independent of the physical tool number.
        offset: u32,
    },
    /// Hold at rest for a finite nonnegative number of seconds.
    Dwell {
        /// Time measured by the task executor after preceding motion is complete.
        seconds: f64,
    },
    /// Drain all pending, active and buffered motion and report a stable endpoint.
    Fence,
    /// Drain and finish; spindle/coolant shutdown must be explicit preceding events.
    End,
}

/// Independently negotiated capabilities. A supported command shape is not evidence
/// that a deployed adapter can execute it. Default is none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Capability {
    /// Native linear motion.
    Linear,
    /// Single-turn planar circular motion.
    PlanarArc,
    /// Geometric blending with explicit deviation budget.
    Blend,
    /// Existing spindle-at-speed entry gate.
    AtSpeed,
    /// Velocity-synchronized feed per revolution.
    FeedPerRevolution,
    /// Task-side spindle start/stop and RPM demands.
    Spindle,
    /// Task/motion CSS coordination.
    Css,
    /// Task-side coolant events.
    Coolant,
    /// Configured tool-change procedure, including synchronization.
    ToolChange,
    /// Independent tool offsets and coordinate rebinding.
    ToolOffset,
    /// Task-side dwell.
    Dwell,
    /// Task/motion completion barriers, including program end.
    Completion,
}

/// In-process capability set; not a wire bitmask or installed-controller discovery.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities(u32);

impl Capabilities {
    /// Advertise one capability after the adapter has established support.
    #[must_use]
    pub const fn with(self, capability: Capability) -> Self {
        Self(self.0 | (1 << capability as u8))
    }

    /// Whether the negotiated set contains a capability.
    #[must_use]
    pub const fn contains(self, capability: Capability) -> bool {
        self.0 & (1 << capability as u8) != 0
    }

    fn require(self, capability: Capability) -> Result<(), ContractError> {
        if self.contains(capability) {
            Ok(())
        } else {
            Err(ContractError::Unsupported(capability))
        }
    }
}

/// Structural/capability failures, before geometry validation or live admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractError {
    /// Coordinate or scalar was NaN/infinite.
    NonFinite,
    /// Required positive/nonnegative scalar was outside its domain.
    InvalidScalar,
    /// Axis or plane is outside the selected machine's contract.
    AxisMismatch,
    /// Planar/full-circle structure is inconsistent.
    ArcStructure,
    /// Rapid arcs are not a v1 command.
    RapidArc,
    /// Only spindle zero is represented in v1.
    SpindleIndex,
    /// The adapter has not advertised this requirement.
    Unsupported(Capability),
    /// Semantic contract revision differs.
    Version,
    /// Zero identifiers or an exhausted sequence number.
    InvalidIdentity,
    /// Job or coordinate-state generation differs.
    StaleBinding,
    /// Only the next sequence may be newly admitted.
    Sequence,
}

fn positive(value: f64) -> Result<(), ContractError> {
    if !value.is_finite() {
        Err(ContractError::NonFinite)
    } else if value <= 0.0 {
        Err(ContractError::InvalidScalar)
    } else {
        Ok(())
    }
}

impl Command {
    /// Check representation and required capabilities only. Success does NOT
    /// certify radius agreement, path continuity, limits, offsets, clearance,
    /// spindle state, producer ownership or permission to enqueue/run.
    pub fn check_structure(
        self,
        machine: Machine,
        capabilities: Capabilities,
    ) -> Result<(), ContractError> {
        let result = match self {
            Self::Motion(motion) => motion.check(machine, capabilities),
            Self::Spindle(spindle) => {
                capabilities.require(Capability::Spindle)?;
                match spindle {
                    Spindle::Stop => Ok(()),
                    Spindle::Rpm { rpm, .. } => positive(rpm),
                    Spindle::Css {
                        surface_mm_per_second,
                        maximum_rpm,
                        ..
                    } => {
                        if machine != Machine::LatheXz {
                            return Err(ContractError::AxisMismatch);
                        }
                        capabilities.require(Capability::Css)?;
                        positive(surface_mm_per_second)?;
                        positive(maximum_rpm)
                    }
                }
            }
            Self::Coolant(_) => capabilities.require(Capability::Coolant),
            Self::ChangeTool { .. } => capabilities.require(Capability::ToolChange),
            Self::ToolOffset { .. } => capabilities.require(Capability::ToolOffset),
            Self::Dwell { seconds } => {
                capabilities.require(Capability::Dwell)?;
                if !seconds.is_finite() {
                    Err(ContractError::NonFinite)
                } else if seconds < 0.0 {
                    Err(ContractError::InvalidScalar)
                } else {
                    Ok(())
                }
            }
            Self::Fence | Self::End => capabilities.require(Capability::Completion),
        };
        result?;
        if !matches!(self, Self::Motion(_)) {
            capabilities.require(Capability::Completion)?;
        }
        Ok(())
    }
}

impl Motion {
    // Exact equality below expresses the representation contract (canonical
    // plane coordinates/coincident endpoints), not a geometric error estimate.
    #[allow(clippy::float_cmp)]
    fn check(self, machine: Machine, capabilities: Capabilities) -> Result<(), ContractError> {
        let (start, end) = match self.geometry {
            Geometry::Line { start, end } => {
                capabilities.require(Capability::Linear)?;
                (start, end)
            }
            Geometry::Arc {
                start,
                end,
                center,
                plane,
                extent,
                ..
            } => {
                capabilities.require(Capability::PlanarArc)?;
                start.check(machine)?;
                end.check(machine)?;
                center.check(machine)?;
                if machine == Machine::LatheXz && plane != Plane::Xz {
                    return Err(ContractError::AxisMismatch);
                }
                if self.feed == Feed::Rapid {
                    return Err(ContractError::RapidArc);
                }
                if plane.normal_coordinate(start) != plane.normal_coordinate(end)
                    || plane.normal_coordinate(start) != plane.normal_coordinate(center)
                    || start == center
                    || end == center
                    || ((start == end) != (extent == ArcExtent::FullCircle))
                {
                    return Err(ContractError::ArcStructure);
                }
                (start, end)
            }
        };
        start.check(machine)?;
        end.check(machine)?;
        match self.feed {
            Feed::Rapid => (),
            Feed::PerSecond(speed) => positive(speed)?,
            Feed::PerRevolution {
                mm_per_rev,
                spindle,
            } => {
                if spindle != 0 {
                    return Err(ContractError::SpindleIndex);
                }
                capabilities.require(Capability::FeedPerRevolution)?;
                positive(mm_per_rev)?;
            }
        }
        if let Termination::Blend { max_deviation_mm } = self.termination {
            capabilities.require(Capability::Blend)?;
            positive(max_deviation_mm)?;
        }
        if self.entry_gate == EntryGate::SpindlesAtSpeed {
            capabilities.require(Capability::AtSpeed)?;
        }
        Ok(())
    }
}

/// Controller-issued context. State epoch changes on coordinate/tool/config
/// changes, re-anchoring and recovery; job epoch changes on abort/new ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    /// Nonzero job generation, never reused during an adapter lifetime.
    pub job_epoch: u64,
    /// Nonzero bound machine-state generation.
    pub state_epoch: u64,
}

/// Immutable command identity. The session separately binds the source/plan hashes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandKey {
    /// Contract revision.
    pub version: u32,
    /// Controller-issued binding.
    pub binding: Binding,
    /// One-based sequence, reserved maximum value prevents wraparound.
    pub sequence: u64,
}

impl CommandKey {
    /// Pure identity check for a NEW offer. Does not deduplicate, reserve queue
    /// space or mutate a cursor. Retries require controller receipt reconciliation.
    pub fn check_next(self, active: Binding, next_sequence: u64) -> Result<(), ContractError> {
        if self.version != CONTRACT_VERSION {
            return Err(ContractError::Version);
        }
        if self.binding.job_epoch == 0
            || self.binding.state_epoch == 0
            || self.sequence == 0
            || self.sequence == u64::MAX
        {
            return Err(ContractError::InvalidIdentity);
        }
        if self.binding != active {
            return Err(ContractError::StaleBinding);
        }
        if self.sequence != next_sequence {
            return Err(ContractError::Sequence);
        }
        Ok(())
    }
}

/// The acknowledgment boundary, distinct from physical completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionStage {
    /// The sole task owner has retained the prepared command in a bounded buffer.
    Task,
    /// The corresponding motion/I/O command has been committed by the controller.
    Controller,
}

/// Failures the future adapter must distinguish without skipping commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// Structural, capability or identity failure.
    Contract(ContractError),
    /// Another producer owns task admission.
    NotOwner,
    /// Disabled, unhomed or unsuitable controller mode/state.
    NotReady,
    /// Abort has closed command admission.
    Aborting,
    /// Fault or untrusted position requires controller recovery.
    Faulted,
    /// Geometry failed the downstream invariant/admissibility check.
    Geometry,
    /// Expected start does not match the bound/previous endpoint.
    StartMismatch,
    /// Configured motion or travel constraints are violated.
    Limits,
    /// Required machine procedure/remap has no supported native integration.
    Procedure,
    /// A reused command identity carries different immutable content.
    IdentityConflict,
}

/// Future adapter receipt outcome. Accepted means owned/queued, NEVER executed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    /// This immutable command was committed once.
    Accepted,
    /// Receipt replay for the SAME immutable payload; must not enqueue it again.
    AlreadyAccepted,
    /// No effect; caller retains the command until capacity is available.
    Backpressure,
    /// No new command effect; surface the failure, do not silently skip it.
    Rejected(Rejection),
}

/// Typed receipt; implementation and persistence belong to the future adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmissionReceipt {
    /// Immutable identity to reconcile.
    pub key: CommandKey,
    /// Which owner acknowledged this command.
    pub stage: AdmissionStage,
    /// Result at that boundary; never a physical-completion claim.
    pub outcome: Admission,
}
