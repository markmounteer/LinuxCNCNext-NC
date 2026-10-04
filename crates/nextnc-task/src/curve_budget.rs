//! Off-RT coupled dynamics for the shim's constant-radius circles/helices.
//!
//! With unit-speed geometry, Cartesian jerk is r' j + 3 r'' v a + r''' v³.
//! Independent scalar and cruise-curvature caps cannot bound that sum. For a
//! helix its Frenet components are (j-k²v³, 3kva, ktv³); acceleration components
//! are (a, kv²). Bound the complete box 0<=v<=V, |a|<=A, |j|<=J, including
//! braking signs. The same immutable limits therefore apply to overrides,
//! measured-spindle demand and planner fault stops; no motor samples are made.
//!
//! A bounded search selects a feasible box by an estimated rest-to-rest S-curve
//! time. This is an allocation heuristic, not a whole-job optimality or actual
//! LinuxCNC interpolation certificate. The latter still requires runtime tests.
use super::{fail, Dynamics, Motion, Result, ScalarDynamics};

const GRID: u32 = 16;
// Round feasible boxes inward, below the separately reserved machine limits.
const INWARD: f64 = 1.0 - 64.0 * f64::EPSILON;

#[derive(Clone, Copy)]
struct Curve {
    length: f64,
    curvature: f64,
    torsion: f64,
    acceleration: f64,
    jerk: f64,
    axial_acceleration: f64,
    axial_jerk: f64,
}

/// Remaining orthogonal component, avoiding limit² overflow and cancellation.
fn residual(limit: f64, used: f64) -> f64 {
    if !used.is_finite() || used < 0.0 || used >= limit {
        return 0.0;
    }
    let ratio = used / limit;
    limit * ((1.0 - ratio) * (1.0 + ratio)).sqrt() * INWARD
}

/// Symmetric, jerk-limited rest-to-rest time, including a possible cruise.
fn duration(length: f64, velocity: f64, acceleration: f64, jerk: f64) -> f64 {
    let ramp = (acceleration / jerk).min((velocity / jerk).sqrt());
    let plateau = (velocity / acceleration - ramp).max(0.0);
    let accelerating = 2.0 * ramp + plateau;
    if length >= velocity * accelerating {
        2.0 * accelerating + (length - velocity * accelerating) / velocity
    } else if length <= 2.0 * acceleration * (acceleration / jerk).powi(2) {
        4.0 * (length / (2.0 * jerk)).cbrt()
    } else {
        let ramp = acceleration / jerk;
        ramp + ramp.hypot(2.0 * (length / acceleration).sqrt())
    }
}

impl Curve {
    fn acceleration_limit(self, velocity: f64, scalar_jerk: f64, scalar_acceleration: f64) -> f64 {
        let v3 = velocity * velocity * velocity;
        let tangential = scalar_jerk + self.curvature * self.curvature * v3;
        let binormal = self.curvature * self.torsion * v3;
        let jerk_room = residual(self.jerk, tangential.hypot(binormal));
        scalar_acceleration
            .min(self.axial_acceleration)
            .min(residual(
                self.acceleration,
                self.curvature * velocity * velocity,
            ))
            .min(jerk_room / (3.0 * self.curvature * velocity))
    }

    fn jerk_limit(self, velocity: f64, acceleration: f64) -> f64 {
        let v3 = velocity * velocity * velocity;
        let normal = 3.0 * self.curvature * velocity * acceleration;
        let binormal = self.curvature * self.torsion * v3;
        (residual(self.jerk, normal.hypot(binormal)) - self.curvature * self.curvature * v3)
            .min(self.axial_jerk)
            * INWARD
    }
}

pub(super) fn allocate(
    machine: Dynamics,
    motion: Motion,
    original: ScalarDynamics,
) -> Result<ScalarDynamics> {
    let circle = motion
        .circular
        .ok_or_else(|| fail("missing analytic curve geometry"))?;
    let (u, v, n) = nextnc_native::geometry::basis(circle.plane);
    let radius =
        (motion.start_mm[u] - circle.center_mm[u]).hypot(motion.start_mm[v] - circle.center_mm[v]);
    let rise = (motion.end_mm[n] - motion.start_mm[n]).abs();
    let pitch = rise / circle.sweep_radians;
    let scale = radius.hypot(pitch);
    let length = (radius * circle.sweep_radians).hypot(rise);
    let curvature = (radius / scale) / scale;
    let torsion = (pitch / scale) / scale;
    let axial_fraction = pitch / scale;
    if [radius, scale, length, curvature]
        .iter()
        .any(|x| !x.is_finite() || *x <= 0.0)
        || !torsion.is_finite()
        || !axial_fraction.is_finite()
        || [
            original.maximum_velocity_mm_s,
            original.acceleration_mm_s2,
            original.jerk_mm_s3,
        ]
        .iter()
        .any(|x| !x.is_finite() || *x <= 0.0)
    {
        return Err(fail("unrepresentable analytic-curve dynamics"));
    }
    let curve = Curve {
        length,
        curvature,
        torsion,
        acceleration: machine
            .trajectory
            .acceleration_mm_s2
            .min(machine.axes[u].acceleration_mm_s2)
            .min(machine.axes[v].acceleration_mm_s2),
        jerk: machine
            .trajectory
            .jerk_mm_s3
            .min(machine.axes[u].jerk_mm_s3)
            .min(machine.axes[v].jerk_mm_s3),
        // The absent axis contributes nothing on a planar lathe arc. A helix's
        // axial acceleration/jerk comes only from its constant tangent share.
        axial_acceleration: if axial_fraction > 0.0 {
            machine.axes[n].acceleration_mm_s2 / axial_fraction
        } else {
            f64::INFINITY
        },
        axial_jerk: if axial_fraction > 0.0 {
            machine.axes[n].jerk_mm_s3 / axial_fraction
        } else {
            f64::INFINITY
        },
    };
    let third_derivative = curvature.hypot(torsion) * curvature;
    let ceiling = original
        .maximum_velocity_mm_s
        .min((curve.acceleration / curvature).sqrt())
        .min((curve.jerk / third_derivative).cbrt())
        * INWARD;
    let mut best: Option<(f64, ScalarDynamics)> = None;
    // Fixed work: at most 16 * 17 candidates per source arc, entirely off RT.
    // The acceleration-bound candidate avoids a fixed fractional jerk loss on
    // gentle curves when the machine's acceleration limit binds first.
    for speed_step in 1..=GRID {
        let velocity = ceiling * f64::from(speed_step) / f64::from(GRID);
        let max_jerk = original.jerk_mm_s3.min(curve.jerk_limit(velocity, 0.0));
        let max_acceleration = original
            .acceleration_mm_s2
            .min(curve.axial_acceleration)
            .min(residual(
                curve.acceleration,
                curvature * velocity * velocity,
            ));
        for jerk_step in 0..=GRID {
            let jerk = if jerk_step == 0 {
                max_jerk.min(curve.jerk_limit(velocity, max_acceleration))
            } else {
                max_jerk * f64::from(jerk_step) / f64::from(GRID)
            } * INWARD;
            let acceleration =
                curve.acceleration_limit(velocity, jerk, original.acceleration_mm_s2) * INWARD;
            if [velocity, acceleration, jerk]
                .iter()
                .any(|x| !x.is_finite() || *x <= 0.0)
            {
                continue;
            }
            let time = duration(curve.length, velocity, acceleration, jerk);
            if time.is_finite() && time > 0.0 && best.is_none_or(|(previous, _)| time < previous) {
                best = Some((
                    time,
                    ScalarDynamics {
                        velocity_mm_s: original.velocity_mm_s.min(velocity),
                        maximum_velocity_mm_s: velocity,
                        acceleration_mm_s2: acceleration,
                        jerk_mm_s3: jerk,
                    },
                ));
            }
        }
    }
    best.map(|(_, limits)| limits)
        .ok_or_else(|| fail("no representable coupled analytic-curve dynamics"))
}

#[cfg(test)]
#[path = "curve_budget_tests.rs"]
mod tests;
