//! Native task messages in canonical mm, before the thin LinuxCNC C++ adapter.
//! No interpreter text, planner queue writes or live execution permission.
use crate::binding::{BoundAction, BoundPlan, Motion};
use motion_command::{Feed, Rotation, Termination};
use std::{f64::consts::TAU, ops::Range};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisDynamics {
    pub velocity_mm_s: f64,
    pub acceleration_mm_s2: f64,
    pub jerk_mm_s3: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dynamics {
    /// XYZ or XZ only; an absent axis is not assigned invented limits.
    pub axis_mask: u32,
    pub axes: [AxisDynamics; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalarDynamics {
    pub velocity_mm_s: f64,
    pub maximum_velocity_mm_s: f64,
    pub acceleration_mm_s2: f64,
    pub jerk_mm_s3: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Payload {
    Termination(Termination),
    Motion {
        motion: Motion,
        dynamics: ScalarDynamics,
        turn: Option<i32>,
    },
    /// Keep source identity and entry conditions without sending a degenerate
    /// line to the planner. This requires an explicit task application receipt.
    Stationary(Motion),
    State(BoundAction),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub command: usize,
    pub ordinal: usize,
    pub payload: Payload,
}

#[derive(Debug)]
pub struct Plan {
    pieces: Vec<Piece>,
    commands: Vec<Range<usize>>,
    drains_before: Vec<usize>,
}
impl Plan {
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }
    pub fn commands(&self) -> &[Range<usize>] {
        &self.commands
    }
    /// Includes bound shaper lane changes and termination-policy changes.
    pub fn drains_before(&self) -> &[usize] {
        &self.drains_before
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub command: Option<usize>,
    pub reason: &'static str,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native lowering at {:?}: {}", self.command, self.reason)
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
fn fail(reason: &'static str) -> Error {
    Error {
        command: None,
        reason,
    }
}

impl Dynamics {
    fn validate(self) -> Result<()> {
        if self.axis_mask != 7 && self.axis_mask != 5 {
            return Err(fail("only XYZ/XZ dynamics are supported"));
        }
        for (i, axis) in self.axes.iter().enumerate() {
            for value in [axis.velocity_mm_s, axis.acceleration_mm_s2, axis.jerk_mm_s3] {
                if !value.is_finite()
                    || value < 0.0
                    || (self.axis_mask & (1 << i) != 0 && value == 0.0)
                {
                    return Err(fail("active-axis dynamics must be finite and positive"));
                }
            }
        }
        Ok(())
    }

    fn resolve(self, motion: Motion) -> Result<(ScalarDynamics, Option<i32>)> {
        let delta: [f64; 3] =
            std::array::from_fn(|i| (motion.end_mm[i] - motion.start_mm[i]).abs());
        if delta
            .iter()
            .enumerate()
            .any(|(i, d)| *d != 0.0 && self.axis_mask & (1 << i) == 0)
        {
            return Err(fail("motion uses an unconfigured axis"));
        }
        let mut velocity_time: f64 = 0.0;
        let mut accel_ratio: f64 = 0.0;
        let mut jerk_ratio: f64 = 0.0;
        for (d, axis) in delta.iter().zip(self.axes) {
            if *d > 0.0 {
                velocity_time = velocity_time.max(d / axis.velocity_mm_s);
                accel_ratio = accel_ratio.max(d / axis.acceleration_mm_s2);
                jerk_ratio = jerk_ratio.max(d / axis.jerk_mm_s3);
            }
        }
        let (length, jerk, turn) = if let Some(circle) = motion.circular {
            let (u, v, n) = nextnc_native::geometry::basis(circle.plane);
            if (self.axis_mask & (1 << u) == 0) || (self.axis_mask & (1 << v) == 0) {
                return Err(fail("circle uses an unconfigured plane"));
            }
            let start_u = motion.start_mm[u] - circle.center_mm[u];
            let start_v = motion.start_mm[v] - circle.center_mm[v];
            let end_u = motion.end_mm[u] - circle.center_mm[u];
            let end_v = motion.end_mm[v] - circle.center_mm[v];
            let start_radius = start_u.hypot(start_v);
            let end_radius = end_u.hypot(end_v);
            let radius = start_radius.min(end_radius);
            let spiral = end_radius - start_radius;
            let angle = circle.sweep_radians;
            let planar_length = (radius * angle).hypot(spiral);
            let length = planar_length.hypot(motion.end_mm[n] - motion.start_mm[n]);
            let planar_accel = self.axes[u]
                .acceleration_mm_s2
                .min(self.axes[v].acceleration_mm_s2);
            let effective_radius = radius.hypot(spiral / angle);
            // Match the pinned canonical centripetal allocation and chord-axis
            // limits. The planner still applies its own trajectory/joint guards.
            let planar_velocity = (planar_accel * 3f64.sqrt() / 2.0 * effective_radius)
                .sqrt()
                .min(self.axes[u].velocity_mm_s)
                .min(self.axes[v].velocity_mm_s);
            velocity_time = velocity_time.max(planar_length / planar_velocity);
            accel_ratio = accel_ratio.max(planar_length / planar_accel);
            let direction = if circle.rotation == Rotation::Counterclockwise {
                1.0
            } else {
                -1.0
            };
            let mut principal =
                (direction * (end_v.atan2(end_u) - start_v.atan2(start_u))).rem_euclid(TAU);
            if principal <= 1e-12 {
                principal = TAU;
            }
            let full = ((angle - principal) / TAU).round();
            if !full.is_finite()
                || full < 0.0
                || full > f64::from(i32::MAX - 1)
                || (principal + full * TAU - angle).abs() > 1e-8 * angle.max(1.0)
            {
                return Err(fail(
                    "circle sweep cannot be represented by pinned task turn count",
                ));
            }
            let turns = full as i32;
            let turn = if direction > 0.0 { turns } else { -turns - 1 };
            (
                length,
                self.axes[u].jerk_mm_s3.min(self.axes[v].jerk_mm_s3),
                Some(turn),
            )
        } else {
            let length = delta[0].hypot(delta[1]).hypot(delta[2]);
            (length, length / jerk_ratio, None)
        };
        let maximum_velocity_mm_s = length / velocity_time;
        let velocity_mm_s = match motion.feed {
            Feed::Rapid => maximum_velocity_mm_s,
            Feed::PerSecond(rate) => rate.min(maximum_velocity_mm_s),
            Feed::PerRevolution { .. } => return Err(fail("synchronized feed requires Stage 4")),
        };
        let dynamics = ScalarDynamics {
            velocity_mm_s,
            maximum_velocity_mm_s,
            acceleration_mm_s2: length / accel_ratio,
            jerk_mm_s3: jerk,
        };
        if [
            dynamics.velocity_mm_s,
            dynamics.maximum_velocity_mm_s,
            dynamics.acceleration_mm_s2,
            dynamics.jerk_mm_s3,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0)
        {
            return Err(fail("nonfinite or zero scalar motion dynamics"));
        }
        Ok((dynamics, turn))
    }
}

pub fn lower(bound: &BoundPlan, dynamics: Dynamics) -> Result<Plan> {
    dynamics.validate()?;
    let mut plan = Plan {
        pieces: Vec::new(),
        commands: Vec::new(),
        drains_before: Vec::new(),
    };
    let mut termination = None;
    for record in bound.records() {
        let start = plan.pieces.len();
        let mut drain = record.drain_before;
        match record.action {
            BoundAction::Motion(motion) => {
                if termination != Some(motion.termination) {
                    drain |= termination.is_some();
                    plan.pieces.push(Piece {
                        command: record.command,
                        ordinal: 0,
                        payload: Payload::Termination(motion.termination),
                    });
                    termination = Some(motion.termination);
                }
                let payload = if motion.circular.is_none() && motion.start_mm == motion.end_mm {
                    Payload::Stationary(motion)
                } else {
                    let (dynamics, turn) = dynamics.resolve(motion).map_err(|e| Error {
                        command: Some(record.command),
                        ..e
                    })?;
                    Payload::Motion {
                        motion,
                        dynamics,
                        turn,
                    }
                };
                plan.pieces.push(Piece {
                    command: record.command,
                    ordinal: plan.pieces.len() - start,
                    payload,
                });
            }
            action => plan.pieces.push(Piece {
                command: record.command,
                ordinal: 0,
                payload: Payload::State(action),
            }),
        }
        if drain {
            plan.drains_before.push(record.command);
        }
        plan.commands.push(start..plan.pieces.len());
    }
    Ok(plan)
}
