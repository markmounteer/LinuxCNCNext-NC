//! Off-thread numerical headroom for the pinned rate-one cubic pipeline.
//!
//! This reserves planning demand below physical ceilings. It is not a proof of
//! every profile/recovery implementation: independent continuous-path/dynamics
//! qualification remains necessary, including changed controls and shaping.
use super::{fail, AxisDynamics, Dynamics, Result};
use crate::binding::{BoundAction, BoundPlan};
use motion_command::Termination;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericalBudget {
    pub interpolation_period_ns: u32,
    pub maximum_coordinate_mm: f64,
    pub scalar_horizon_mm: f64,
    pub position_reserve_mm: f64,
    pub velocity_reserve_mm_s: f64,
    pub acceleration_reserve_mm_s2: f64,
    pub jerk_reserve_mm_s3: f64,
}

impl NumericalBudget {
    pub(super) fn for_job(bound: &BoundPlan, machine: Dynamics) -> Result<Self> {
        let mut coordinate = bound.initial_pose_mm()[..3]
            .iter()
            .fold(0.0_f64, |m, p| m.max(p.abs()));
        let mut horizon = machine.scalar_origin_mm;
        for record in bound.records() {
            let BoundAction::Motion(m) = record.action else {
                continue;
            };
            let deviation = match m.termination {
                Termination::Blend { max_deviation_mm } => max_deviation_mm,
                _ => 0.0,
            };
            for i in 0..3 {
                coordinate = coordinate
                    .max(m.start_mm[i].abs() + deviation)
                    .max(m.end_mm[i].abs() + deviation);
            }
            let length_bound = if let Some(c) = m.circular {
                let (u, v, n) = nextnc_native::geometry::basis(c.plane);
                let r0 = (m.start_mm[u] - c.center_mm[u]).hypot(m.start_mm[v] - c.center_mm[v]);
                let r1 = (m.end_mm[u] - c.center_mm[u]).hypot(m.end_mm[v] - c.center_mm[v]);
                let radius = r0.max(r1);
                coordinate = coordinate
                    .max(c.center_mm[u].abs() + radius + deviation)
                    .max(c.center_mm[v].abs() + radius + deviation);
                radius * c.sweep_radians + (r1 - r0).abs() + (m.end_mm[n] - m.start_mm[n]).abs()
            } else {
                (0..3).map(|i| (m.end_mm[i] - m.start_mm[i]).abs()).sum()
            };
            // L1 line/arc bounds also cover a shorter fitted blend. Round upward
            // at each accumulation instead of underestimating the scalar scale.
            horizon = (horizon + length_bound + 2.0 * deviation).next_up();
        }
        let dt = f64::from(machine.interpolation_period_ns) * 1e-9;
        // The cubic operation tree forms weighted positions, then subtracts
        // near-equal values and divides by dt, dt^2 and dt^3. Keep a 256-epsilon
        // positional allowance at the observed coordinate/scalar scale. The
        // finite-difference coefficient sums are 2, 4 and 8 respectively. This
        // explicit allowance exceeds simple one-ULP input quantization; its
        // sufficiency for the real pipeline must still be measured, not assumed.
        let position = (256.0 * f64::EPSILON * coordinate.max(horizon).max(1.0)).next_up();
        let result = Self {
            interpolation_period_ns: machine.interpolation_period_ns,
            maximum_coordinate_mm: coordinate,
            scalar_horizon_mm: horizon,
            position_reserve_mm: position,
            velocity_reserve_mm_s: (2.0 * position / dt).next_up(),
            acceleration_reserve_mm_s2: (4.0 * position / dt.powi(2)).next_up(),
            jerk_reserve_mm_s3: (8.0 * position / dt.powi(3)).next_up(),
        };
        if [
            coordinate,
            horizon,
            result.position_reserve_mm,
            result.velocity_reserve_mm_s,
            result.acceleration_reserve_mm_s2,
            result.jerk_reserve_mm_s3,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(fail("unrepresentable native numerical reserve"));
        }
        Ok(result)
    }

    fn axis(self, physical: AxisDynamics) -> Result<AxisDynamics> {
        let result = AxisDynamics {
            velocity_mm_s: (physical.velocity_mm_s - self.velocity_reserve_mm_s).next_down(),
            acceleration_mm_s2: (physical.acceleration_mm_s2 - self.acceleration_reserve_mm_s2)
                .next_down(),
            jerk_mm_s3: (physical.jerk_mm_s3 - self.jerk_reserve_mm_s3).next_down(),
        };
        if [
            result.velocity_mm_s,
            result.acceleration_mm_s2,
            result.jerk_mm_s3,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0)
        {
            return Err(fail(
                "native numerical reserve exhausts a physical dynamics limit",
            ));
        }
        Ok(result)
    }

    pub(super) fn reserve(self, mut machine: Dynamics) -> Result<Dynamics> {
        machine.trajectory = self.axis(machine.trajectory)?;
        for (i, axis) in machine.axes.iter_mut().enumerate() {
            if machine.axis_mask & (1 << i) != 0 {
                *axis = self.axis(*axis)?;
            }
        }
        Ok(machine)
    }
}
