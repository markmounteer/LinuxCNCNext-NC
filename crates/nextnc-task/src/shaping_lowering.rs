//! Source-history budgets for common XY FIR execution. All work is off-thread.
//! Positive reviewed execution allowance is spent once: on downstream shaping.
//! Planner fitting is disabled within these charts so retained source arclength
//! remains the scalar speed's parameter. CAM tolerance is never spent here.
use super::{fail, Dynamics, Error, NumericalBudget, Payload, Plan, Result};
use crate::binding::{BoundAction, BoundPlan, BoundRecord, Motion};
use crate::shaping::{Allowance, Certificate, Geometry, Kernel};
use motion_command::{Feed, Termination};
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct ShapingBudget {
    pub commands: Range<usize>,
    pub certificate: Certificate,
    pub maximum_history_source_span_mm: f64,
}

#[derive(Clone, Copy, PartialEq)]
enum Chart {
    Polyline,
    Circle { center: [f64; 3], z: f64 },
}

fn chart(m: Motion) -> Option<Chart> {
    if m.feed == Feed::Rapid || (m.circular.is_none() && m.start_mm[..2] == m.end_mm[..2]) {
        return None;
    }
    Some(if let Some(c) = m.circular {
        Chart::Circle {
            center: c.center_mm,
            z: m.start_mm[2],
        }
    } else {
        Chart::Polyline
    })
}

fn allowance(m: Motion) -> f64 {
    match m.termination {
        Termination::Blend { max_deviation_mm } => max_deviation_mm,
        _ => 0.0,
    }
}

#[derive(Clone, Copy)]
struct Corner {
    /// Outward bounds for cumulative source length at this junction.
    position: [f64; 2],
    jump_prefix: [f64; 2],
}

fn add_interval(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    if a == [0.0; 2] {
        return b;
    }
    if b == [0.0; 2] {
        return a;
    }
    [(a[0] + b[0]).next_down().max(0.0), (a[1] + b[1]).next_up()]
}

fn difference(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    if a == b && a[0] == a[1] {
        [0.0; 2]
    } else {
        [(a[0] - b[1]).next_down(), (a[1] - b[0]).next_up()]
    }
}

fn norm(v: [[f64; 2]; 3]) -> [f64; 2] {
    let squared = v.into_iter().fold([0.0; 2], |sum, [lo, hi]| {
        let near = if lo <= 0.0 && hi >= 0.0 {
            0.0
        } else {
            lo.abs().min(hi.abs())
        };
        let far = lo.abs().max(hi.abs());
        add_interval(
            sum,
            [
                (near * near).next_down().max(0.0),
                if far == 0.0 {
                    0.0
                } else {
                    (far * far).next_up()
                },
            ],
        )
    });
    [
        squared[0].sqrt().next_down().max(0.0),
        if squared[1] == 0.0 {
            0.0
        } else {
            squared[1].sqrt().next_up()
        },
    ]
}

fn delta(m: Motion) -> [[f64; 2]; 3] {
    std::array::from_fn(|i| difference([m.end_mm[i]; 2], [m.start_mm[i]; 2]))
}

// A coordinate-axis direction can be established exactly without dividing two
// approximate norms. General directions keep their full interval uncertainty.
fn axis_direction(v: [[f64; 2]; 3]) -> Option<[f64; 3]> {
    let active: Vec<_> = v
        .iter()
        .enumerate()
        .filter(|(_, x)| **x != [0.0; 2])
        .collect();
    if let [(axis, [lo, hi])] = active.as_slice() {
        if *lo > 0.0 || *hi < 0.0 {
            let mut result = [0.0; 3];
            result[*axis] = lo.signum();
            return Some(result);
        }
    }
    None
}

fn corners(motions: &[Motion]) -> Result<Vec<Corner>> {
    let mut result = Vec::with_capacity(motions.len());
    let (mut distance, mut total) = ([0.0; 2], [0.0; 2]);
    let mut previous: Option<[[f64; 2]; 3]> = None;
    for m in motions {
        let delta = delta(*m);
        let length = norm(delta);
        if !length[1].is_finite() || length[0] <= 0.0 {
            return Err(fail("invalid shaped source span"));
        }
        let unit = if let Some(axis) = axis_direction(delta) {
            axis.map(|x| [x; 2])
        } else {
            delta.map(|[lo, hi]| {
                let values = [
                    lo / length[0],
                    lo / length[1],
                    hi / length[0],
                    hi / length[1],
                ];
                [
                    values
                        .into_iter()
                        .fold(f64::INFINITY, f64::min)
                        .next_down()
                        .max(-1.0),
                    values
                        .into_iter()
                        .fold(f64::NEG_INFINITY, f64::max)
                        .next_up()
                        .min(1.0),
                ]
            })
        };
        if let Some(before) = previous {
            let jump = norm(std::array::from_fn(|i| difference(unit[i], before[i])));
            total = add_interval(total, [jump[0].min(2.0), jump[1].min(2.0)]);
            result.push(Corner {
                position: distance,
                jump_prefix: total,
            });
        }
        previous = Some(unit);
        distance = add_interval(distance, length);
    }
    if distance.iter().chain(total.iter()).any(|v| !v.is_finite()) {
        return Err(fail("unrepresentable shaped source chart"));
    }
    Ok(result)
}

fn window_jumps(corners: &[Corner], span: f64) -> f64 {
    let (mut left, mut maximum) = (0, 0.0_f64);
    for (right, corner) in corners.iter().enumerate() {
        while left < right && (corner.position[0] - corners[left].position[1]).next_down() > span {
            left += 1;
        }
        let prefix_lower = if left == 0 {
            0.0
        } else {
            corners[left - 1].jump_prefix[0]
        };
        let sum = corner.jump_prefix[1] - prefix_lower;
        maximum = maximum.max(if sum == 0.0 { 0.0 } else { sum.next_up() });
    }
    maximum
}

fn finish(
    bound: &BoundPlan,
    records: &mut [BoundRecord],
    range: Range<usize>,
    machine: Dynamics,
    numeric: NumericalBudget,
    kernel: &Kernel,
    budgets: &mut Vec<ShapingBudget>,
) -> Result<()> {
    let motions: Vec<_> = bound.records()[range.clone()]
        .iter()
        .filter_map(|r| {
            if let BoundAction::Motion(m) = r.action {
                chart(m).map(|_| m)
            } else {
                None
            }
        })
        .collect();
    if motions.is_empty() {
        return Ok(());
    }
    let first = motions[0];
    let corridor = allowance(first);
    let mut ceiling = f64::INFINITY;
    for m in &motions {
        ceiling = ceiling.min(machine.resolve(*m)?.0.maximum_velocity_mm_s);
    }
    let mut radius = [f64::INFINITY, 0.0_f64];
    if let Some(c) = first.circular {
        for m in &motions {
            for p in [m.start_mm, m.end_mm] {
                let r = norm([
                    difference([p[0]; 2], [c.center_mm[0]; 2]),
                    difference([p[1]; 2], [c.center_mm[1]; 2]),
                    [0.0; 2],
                ]);
                radius[0] = radius[0].min(r[0]);
                radius[1] = radius[1].max(r[1]);
            }
        }
        if radius[0] <= 0.0 || !radius[1].is_finite() {
            return Err(fail("invalid shaped circle radius"));
        }
    }
    let atoms = if first.circular.is_none() {
        corners(&motions)?
    } else {
        Vec::new()
    };
    let curvature = if first.circular.is_some() {
        (1.0 / radius[0]).next_up()
    } else {
        0.0
    };
    let nonstraight = curvature > 0.0 || atoms.iter().any(|c| c.jump_prefix[1] > 0.0);
    if corridor == 0.0 {
        if nonstraight {
            return Err(fail("input shaping changes this exact path but no additional execution deviation was approved; CAM tolerance cannot be spent again"));
        }
        // A positive average on a single straight source chart stays on that
        // chart. Its numerical/coefficient errors remain qualification duties.
        return Ok(());
    }
    let ledger = Allowance {
        source_corridor_mm: corridor,
        // Both raw radius variation and projection back to the supplied circle
        // are enclosed. No planner fit/blend consumes additional error here.
        upstream_error_mm: if first.circular.is_some() {
            (2.0 * (radius[1] - radius[0])).next_up()
        } else {
            0.0
        },
        arithmetic_error_mm: numeric.position_reserve_mm,
        maximum_position_norm_mm: (numeric.maximum_coordinate_mm * 3.0_f64.sqrt()).next_up(),
    };
    let duration =
        (f64::from(kernel.history_ticks()) * f64::from(kernel.period_ns()) / 1e9).next_up();
    let span = |v: f64| ((v * duration).next_up() + 2.0 * numeric.position_reserve_mm).next_up();
    let geometry = |v: f64| Geometry {
        curvature_per_mm: curvature,
        tangent_jump_sum: window_jumps(&atoms, span(v)),
    };
    // Smaller V also shortens the set of corners visible in retained history.
    // Search that monotone coupled bound, rather than limiting a whole job by
    // all of its corners or by the history span at the original machine speed.
    let fits = |v| {
        kernel
            .allocate(geometry(v), ledger, v)
            .is_ok_and(|c| c.maximum_velocity_mm_s == v)
    };
    let cap = if fits(ceiling) {
        ceiling
    } else {
        let (mut lo, mut hi) = (0.0, ceiling);
        for _ in 0..64 {
            let mid = lo + (hi - lo) * 0.5;
            if fits(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    };
    let certificate = kernel
        .allocate(geometry(cap), ledger, cap)
        .map_err(|e| fail(e.0))?;
    for r in &mut records[range.clone()] {
        if let BoundAction::Motion(ref mut m) = r.action {
            // Preserve source policy in r.source. The approved allowance is
            // spent downstream, so the planner must not spend it a second time.
            if chart(*m).is_some() {
                m.termination = Termination::ExactPath;
            }
        }
    }
    budgets.push(ShapingBudget {
        commands: range,
        certificate,
        maximum_history_source_span_mm: span(cap),
    });
    Ok(())
}

pub(super) fn prepare(
    bound: &BoundPlan,
    machine: Dynamics,
    numeric: NumericalBudget,
) -> Result<(Vec<BoundRecord>, Vec<ShapingBudget>)> {
    let mut records = bound.records().to_vec();
    let Some(kernel) = bound.shaping_kernel() else {
        return Ok((records, Vec::new()));
    };
    let (mut start, mut active, mut previous_end, mut budget) = (None, None, None, 0.0);
    let mut budgets = Vec::new();
    for i in 0..records.len() {
        let motion = if let BoundAction::Motion(m) = records[i].action {
            Some(m)
        } else {
            None
        };
        let next = motion.and_then(chart);
        let incompatible = records[i].drain_before
            || motion.is_some_and(|m| {
                next != active
                    || allowance(m) != budget
                    || previous_end.is_some_and(|end| end != m.start_mm)
            });
        if let Some(begin) = start {
            if incompatible {
                finish(
                    bound,
                    &mut records,
                    begin..i,
                    machine,
                    numeric,
                    kernel,
                    &mut budgets,
                )
                .map_err(|e| Error {
                    command: Some(begin),
                    ..e
                })?;
                // A cap/geometry transfer only follows actual planner/FIR/cubic
                // drain. No source label or stop-planning flag stands in for it.
                records[i].drain_before = true;
                start = None;
            }
        }
        if let Some(m) = motion {
            if next.is_some() && start.is_none() {
                start = Some(i);
            }
            active = next;
            budget = allowance(m);
            previous_end = Some(m.end_mm);
        }
    }
    if let Some(begin) = start {
        let end = records.len();
        finish(
            bound,
            &mut records,
            begin..end,
            machine,
            numeric,
            kernel,
            &mut budgets,
        )
        .map_err(|e| Error {
            command: Some(begin),
            ..e
        })?;
    }
    Ok((records, budgets))
}

pub(super) fn apply(plan: &mut Plan) {
    for budget in &plan.shaping_budgets {
        for command in budget.commands.clone() {
            for piece in &mut plan.pieces[plan.commands[command].clone()] {
                if let Payload::Motion { dynamics, .. } = &mut piece.payload {
                    dynamics.maximum_velocity_mm_s = dynamics
                        .maximum_velocity_mm_s
                        .min(budget.certificate.maximum_velocity_mm_s);
                    dynamics.velocity_mm_s =
                        dynamics.velocity_mm_s.min(dynamics.maximum_velocity_mm_s);
                }
            }
        }
    }
}
