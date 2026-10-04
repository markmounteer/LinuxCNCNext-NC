//! Off-thread executable line-run limits for the observed rate-one cubic model.
//! The planner still owns profiles, override/feed synchronization and braking.
//! These algebraic budgets require independent full-stack path qualification;
//! they do not certify arbitrary controls or the shaped continuous trajectory.
use super::{fail, Dynamics, Error, Payload, Plan, Result};
use crate::binding::{BoundAction, BoundRecord, Motion};
use motion_command::{EntryGate, Feed, Termination};
use nextnc_native::compiled::{Action, Site};
use std::ops::Range;

// The qualified planner preserves junctions below this cosine as separate
// scalar moves. Match its rate-one line-coalescing boundary: such a corner
// must not impose one reduced speed on both otherwise independent runs.
const JUNCTION_COS_MERGE: f64 = 1.0 - 1e-6;

#[derive(Clone, Debug, PartialEq)]
pub struct CornerBudget {
    /// Original command interval; no commands are merged or renumbered.
    pub commands: Range<usize>,
    pub interpolation_period_ns: u32,
    pub maximum_direction_jump: f64,
    pub minimum_span_mm: f64,
    pub maximum_velocity_mm_s: f64,
    pub acceleration_mm_s2: f64,
    pub jerk_mm_s3: f64,
}

fn eligible(record: &BoundRecord) -> Option<Motion> {
    let (BoundAction::Motion(motion), Action::Motion(_)) = (record.action, record.source.action)
    else {
        return None;
    };
    (record.source.site == Site::Source
        && motion.circular.is_none()
        && motion.start_mm != motion.end_mm
        && motion.feed != Feed::Rapid
        && motion.termination == Termination::ExactPath
        && record.css_update.is_none())
    .then_some(motion)
}

fn adjacent(previous: &BoundRecord, current: &BoundRecord) -> bool {
    let (Some(a), Some(b)) = (eligible(previous), eligible(current)) else {
        return false;
    };
    // Source ordinals, movement labels and CAM tolerances do not reach the
    // planner as stop/merge barriers. A restarted ordinal or separate source
    // line therefore cannot remove a physical corner from this budget. Keep
    // provenance in the unchanged records; group by executable motion instead.
    !current.drain_before
        && b.entry_gate == EntryGate::None
        && a.end_mm == b.start_mm
        && a.feed == b.feed
        && coalescing_corner(a, b)
}

fn coalescing_corner(a: Motion, b: Motion) -> bool {
    let (Ok((left, _)), Ok((right, _))) = (direction(a), direction(b)) else {
        return false;
    };
    let cosine: f64 = left.iter().zip(right).map(|(x, y)| x * y).sum();
    cosine >= JUNCTION_COS_MERGE
}

fn direction(motion: Motion) -> Result<([f64; 3], f64)> {
    let delta: [f64; 3] = std::array::from_fn(|i| motion.end_mm[i] - motion.start_mm[i]);
    let length = delta[0].hypot(delta[1]).hypot(delta[2]);
    if !length.is_finite() || length <= 0.0 {
        return Err(fail("invalid source span in corner budget"));
    }
    Ok((delta.map(|v| v / length), length))
}

fn finish(
    records: &[BoundRecord],
    range: Range<usize>,
    machine: Dynamics,
    plan: &mut Plan,
) -> Result<()> {
    if range.len() < 2 {
        return Ok(());
    }
    let mut previous: Option<[f64; 3]> = None;
    let mut jump = 0.0_f64;
    let mut minimum_span = f64::INFINITY;
    let mut maximum_velocity = f64::INFINITY;
    let mut acceleration = f64::INFINITY;
    let mut jerk = f64::INFINITY;
    for (axis, limits) in machine.axes.iter().enumerate() {
        if machine.axis_mask & (1 << axis) != 0 {
            acceleration = acceleration.min(limits.acceleration_mm_s2);
            jerk = jerk.min(limits.jerk_mm_s3);
        }
    }
    for record in &records[range.clone()] {
        let Some(motion) = eligible(record) else {
            return Err(fail("incompatible source in corner budget"));
        };
        let (unit, length) = direction(motion)?;
        minimum_span = minimum_span.min(length);
        if let Some(before) = previous {
            jump = jump.max(
                (unit[0] - before[0])
                    .hypot(unit[1] - before[1])
                    .hypot(unit[2] - before[2]),
            );
        }
        previous = Some(unit);
        let (limits, _) = machine.resolve(motion)?;
        maximum_velocity = maximum_velocity.min(limits.maximum_velocity_mm_s);
        acceleration = acceleration.min(limits.acceleration_mm_s2);
        jerk = jerk.min(limits.jerk_mm_s3);
    }
    if jump == 0.0 {
        return Ok(());
    }
    let dt = f64::from(machine.interpolation_period_ns) * 1e-9;
    // Allocate scalar and corner contributions separately. The span bound
    // limits a three-period cubic stencil to at most one source corner.
    let scalar_a = (acceleration / 2.0).min(jerk * dt / (3.0 * jump));
    let scalar_j = jerk / 2.0;
    let cap = maximum_velocity
        .min((acceleration - scalar_a) * dt / jump)
        .min((jerk - scalar_j - 0.75 * jump * scalar_a / dt) * dt * dt / jump)
        .min(minimum_span / (3.0 * dt));
    if [cap, scalar_a, scalar_j]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0)
    {
        return Err(fail("unrepresentable source-corner budget"));
    }
    for command in range.clone() {
        for piece in &mut plan.pieces[plan.commands[command].clone()] {
            if let Payload::Motion { dynamics, .. } = &mut piece.payload {
                dynamics.maximum_velocity_mm_s = dynamics.maximum_velocity_mm_s.min(cap);
                dynamics.velocity_mm_s = dynamics.velocity_mm_s.min(dynamics.maximum_velocity_mm_s);
                dynamics.acceleration_mm_s2 = dynamics.acceleration_mm_s2.min(scalar_a);
                dynamics.jerk_mm_s3 = dynamics.jerk_mm_s3.min(scalar_j);
            }
        }
    }
    plan.corner_budgets.push(CornerBudget {
        commands: range,
        interpolation_period_ns: machine.interpolation_period_ns,
        maximum_direction_jump: jump,
        minimum_span_mm: minimum_span,
        maximum_velocity_mm_s: cap,
        acceleration_mm_s2: scalar_a,
        jerk_mm_s3: scalar_j,
    });
    Ok(())
}

pub(super) fn apply(records: &[BoundRecord], machine: Dynamics, plan: &mut Plan) -> Result<()> {
    let mut start = None;
    for (index, record) in records.iter().enumerate() {
        if let Some(first) = start {
            if !adjacent(&records[index - 1], record) {
                finish(records, first..index, machine, plan).map_err(|e| Error {
                    command: Some(first),
                    ..e
                })?;
                start = None;
            }
        }
        if start.is_none() && eligible(record).is_some() {
            start = Some(index);
        }
    }
    if let Some(first) = start {
        finish(records, first..records.len(), machine, plan).map_err(|e| Error {
            command: Some(first),
            ..e
        })?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "corner_budget_tests.rs"]
mod tests;
