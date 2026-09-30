//! Revision 2 semantic vocabulary. This module does not execute commands.
//!
//! Geometry is expressed in Cartesian millimetres in the coordinate space
//! explicitly named by the containing plan/binding. Prepared work coordinates
//! are not resolved machine coordinates and cannot be admitted without binding.
//! Radius, endpoint/sweep agreement and continuous-path bounds are independently
//! checked by the non-real-time compiler; these checks only validate structure.

use crate::{
    Capabilities, Capability, ContractError, EntryGate, Feed, Machine, Plane, PointMm, Rotation,
    Termination,
};

/// Independent of the retained revision-1 vocabulary and of LinuxCNC's ABI.
pub const CONTRACT_VERSION: u32 = 2;

/// Source movement purpose. It is never inferred from a feed magnitude.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Movement {
    /// Older source or source engine did not supply movement purpose.
    Unspecified,
    /// Positioning at machine rapid limits.
    Rapid,
    /// Cutting engagement.
    Cutting,
    /// Finishing engagement.
    FinishCutting,
    /// Lead into the cut.
    LeadIn,
    /// Lead out of the cut.
    LeadOut,
    /// Linking transition.
    LinkTransition,
    /// Direct linking path.
    LinkDirect,
    /// Helical ramp.
    RampHelix,
    /// Profile ramp.
    RampProfile,
    /// Zig-zag ramp.
    RampZigZag,
    /// Other ramp.
    Ramp,
    /// Axial plunge.
    Plunge,
    /// Predrilling entry.
    Predrill,
    /// Extended cutting region.
    Extended,
    /// Reduced-feed region.
    Reduced,
    /// CAM high-feed region; still a dimensioned cutting feed.
    HighFeed,
}

/// A tolerance's origin is part of its semantics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tolerance {
    /// Absent source field. No implicit blending/fitting allowance follows.
    Missing,
    /// Fusion's operation:tolerance, converted once to millimetres.
    FusionOperationMm(f64),
    /// Explicit tolerance supplied by another conforming source.
    SourceDeclaredMm(f64),
}

impl Tolerance {
    /// Check the declared CAM allowance without allocating any fit/blend budget.
    pub fn check(self) -> Result<(), ContractError> {
        match self {
            Self::Missing => Ok(()),
            Self::FusionOperationMm(value) | Self::SourceDeclaredMm(value) => {
                super::positive(value)
            }
        }
    }
}

/// Analytic geometry; no samples or controller timing are embedded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geometry {
    /// Straight segment, retaining even an explicitly supplied zero-length use.
    Line {
        /// Expected source start.
        start: PointMm,
        /// Exact supplied endpoint.
        end: PointMm,
    },
    /// Constant-radius circular or helical path. The center is an axis point at
    /// the start's axial height. Sweep is positive; rotation supplies its sign.
    /// Sweep may exceed a complete revolution. Axial rise is signed, independent
    /// of rotation; it is zero for a planar circular path.
    Circular {
        /// Expected source start.
        start: PointMm,
        /// Exact supplied endpoint.
        end: PointMm,
        /// Axis point at start height.
        center: PointMm,
        /// Right-handed basis: XY=(X,Y), XZ=(Z,X), YZ=(Y,Z).
        plane: Plane,
        /// Viewed toward the origin from the positive plane normal.
        rotation: Rotation,
        /// Strictly positive total angular travel, in radians.
        sweep_radians: f64,
        /// End axial coordinate minus start axial coordinate, in millimetres.
        axial_rise_mm: f64,
    },
}

/// Revision-2 requirements, separate from installed runtime support.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeometryCapabilities {
    /// Previously negotiated basic capabilities; none are assumed by default.
    pub base: Capabilities,
    /// Constant-radius simultaneous in-plane and normal-axis motion.
    pub helix: bool,
    /// Sweeps greater than one revolution.
    pub multiple_turns: bool,
}

/// Explicit revision-2 structural failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Revision-1 structural/capability failure.
    Base(ContractError),
    /// Receiver cannot interpret this semantic revision.
    Version,
    /// Axial-rise, radius-vector or plane structure is inconsistent.
    CircularStructure,
    /// Receiver has not established helical execution support.
    HelixUnsupported,
    /// Receiver has not established multi-turn support.
    MultipleTurnsUnsupported,
    /// A rapid purpose was paired with a cutting feed or vice versa.
    MovementFeed,
}

impl From<ContractError> for Error {
    fn from(value: ContractError) -> Self {
        Self::Base(value)
    }
}

/// A source motion plus independent CAM intent and controller exit policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Geometry in the enclosing plan's explicitly named coordinate space.
    pub geometry: Geometry,
    /// Feed with physical dimensions, never a nominal-RPM approximation.
    pub feed: Feed,
    /// Boundary policy with its own explicit deviation allowance.
    pub termination: Termination,
    /// Synchronization gate before entry.
    pub entry_gate: EntryGate,
    /// Source purpose, retained across reductions and source maps.
    pub movement: Movement,
    /// CAM tolerance and provenance, not permission to add geometric error.
    pub tolerance: Tolerance,
}

impl Motion {
    /// Validate structure and negotiation. This does not validate geometry,
    /// offsets, live state, clearance, source continuity or authorize movement.
    #[allow(clippy::float_cmp)] // Exact representation invariants, not geometric estimates.
    pub fn check_structure(
        self,
        version: u32,
        machine: Machine,
        caps: GeometryCapabilities,
    ) -> Result<(), Error> {
        if version != CONTRACT_VERSION {
            return Err(Error::Version);
        }
        self.tolerance.check()?;
        if self.movement != Movement::Unspecified
            && ((self.movement == Movement::Rapid) != (self.feed == Feed::Rapid))
        {
            return Err(Error::MovementFeed);
        }
        let (start, end) = match self.geometry {
            Geometry::Line { start, end } => {
                caps.base.require(Capability::Linear)?;
                (start, end)
            }
            Geometry::Circular {
                start,
                end,
                center,
                plane,
                sweep_radians,
                axial_rise_mm,
                ..
            } => {
                caps.base.require(Capability::PlanarArc)?;
                super::positive(sweep_radians)?;
                start.check(machine)?;
                end.check(machine)?;
                center.check(machine)?;
                if !axial_rise_mm.is_finite() {
                    return Err(ContractError::NonFinite.into());
                }
                if machine == Machine::LatheXz && plane != Plane::Xz {
                    return Err(ContractError::AxisMismatch.into());
                }
                if self.feed == Feed::Rapid {
                    return Err(ContractError::RapidArc.into());
                }
                let start_n = plane.normal_coordinate(start);
                if plane.normal_coordinate(center) != start_n
                    || plane.normal_coordinate(end) - start_n != axial_rise_mm
                    || start == center
                {
                    return Err(Error::CircularStructure);
                }
                if axial_rise_mm != 0.0 && !caps.helix {
                    return Err(Error::HelixUnsupported);
                }
                if sweep_radians > core::f64::consts::TAU && !caps.multiple_turns {
                    return Err(Error::MultipleTurnsUnsupported);
                }
                (start, end)
            }
        };
        // Reuse the revision-1 checks for units, machine axes, feed dimensions,
        // termination budget and entry-gate capabilities without duplicating rules.
        crate::Command::Motion(crate::Motion {
            geometry: crate::Geometry::Line { start, end },
            feed: self.feed,
            termination: self.termination,
            entry_gate: self.entry_gate,
        })
        .check_structure(machine, caps.base.with(Capability::Linear))?;
        Ok(())
    }
}
