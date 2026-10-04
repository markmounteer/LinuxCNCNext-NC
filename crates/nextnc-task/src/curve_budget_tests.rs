use super::*;
use crate::binding::Circular;
use motion_command::{EntryGate, Feed, Plane, Rotation, Termination};

fn machine(asymmetric: bool) -> Dynamics {
    Dynamics {
        axis_mask: 7,
        interpolation_period_ns: 1_000_000,
        scalar_origin_mm: 0.0,
        trajectory: super::super::AxisDynamics {
            velocity_mm_s: 30.,
            acceleration_mm_s2: 100.,
            jerk_mm_s3: 1000.,
        },
        axes: std::array::from_fn(|axis| super::super::AxisDynamics {
            velocity_mm_s: 30.,
            acceleration_mm_s2: if asymmetric {
                [80., 4., 30.][axis]
            } else {
                100.
            },
            jerk_mm_s3: if asymmetric {
                [400., 3., 70.][axis]
            } else {
                1000.
            },
        }),
    }
}

fn motion(plane: Plane, radius: f64, pitch: f64, sweep: f64, clockwise: bool) -> Motion {
    let (u, v, n) = nextnc_native::geometry::basis(plane);
    let sign = if clockwise { -1. } else { 1. };
    let mut start = [0.; 9];
    start[u] = radius;
    let mut end = [0.; 9];
    end[u] = radius * sweep.cos();
    end[v] = sign * radius * sweep.sin();
    end[n] = pitch * sweep;
    Motion {
        start_mm: start,
        end_mm: end,
        circular: Some(Circular {
            center_mm: [0.; 3],
            plane,
            rotation: if clockwise {
                Rotation::Clockwise
            } else {
                Rotation::Counterclockwise
            },
            sweep_radians: sweep,
            axial_rise_mm: pitch * sweep,
        }),
        feed: Feed::PerSecond(5.),
        termination: Termination::ExactPath,
        entry_gate: EntryGate::None,
    }
}

fn requested() -> ScalarDynamics {
    ScalarDynamics {
        velocity_mm_s: 5.,
        maximum_velocity_mm_s: 30.,
        acceleration_mm_s2: 100.,
        jerk_mm_s3: 1000.,
    }
}

/// Differentiate Cartesian R cos(theta), R sin(theta), pitch*theta directly.
/// No allocator curvature/Frenet expressions are used in this physical oracle.
fn cartesian_derivatives(
    radius: f64,
    pitch: f64,
    phase: f64,
    direction: f64,
    v: f64,
    a: f64,
    j: f64,
) -> ([f64; 3], [f64; 3]) {
    let length_per_radian = radius.hypot(pitch);
    let omega = direction * v / length_per_radian;
    let alpha = direction * a / length_per_radian;
    let beta = direction * j / length_per_radian;
    let (s, c) = phase.sin_cos();
    (
        [
            radius * (-c * omega * omega - s * alpha),
            radius * (-s * omega * omega + c * alpha),
            pitch * direction * alpha,
        ],
        [
            radius * (s * omega.powi(3) - 3. * c * omega * alpha - s * beta),
            radius * (-c * omega.powi(3) - 3. * s * omega * alpha + c * beta),
            pitch * direction * beta,
        ],
    )
}

#[test]
fn original_scalar_limits_have_a_real_cartesian_braking_violation() -> Result<()> {
    let (_, jerk) = cartesian_derivatives(1., 0., 0.5, 1., 5., -50., -1000.);
    assert!(jerk[0].hypot(jerk[1]).hypot(jerk[2]) > 1300.);
    let limits = allocate(
        machine(false),
        motion(Plane::Xy, 1., 0., std::f64::consts::FRAC_PI_2, false),
        requested(),
    )?;
    assert!(
        limits.maximum_velocity_mm_s > 0.5
            && limits.acceleration_mm_s2 > 5.
            && limits.jerk_mm_s3 > 50.
    );
    Ok(())
}

#[test]
fn boxes_bound_cartesian_acceleration_and_jerk_in_all_planes_and_signs() -> Result<()> {
    for plane in [Plane::Xy, Plane::Xz, Plane::Yz] {
        let (u, v, n) = nextnc_native::geometry::basis(plane);
        for asymmetric in [false, true] {
            let machine = machine(asymmetric);
            for radius in [0.001, 0.1, 1., 50., 1000.] {
                for pitch_ratio in [0., 0.01, -0.5, 5.] {
                    let pitch = radius * pitch_ratio;
                    for sweep in [std::f64::consts::FRAC_PI_2, std::f64::consts::TAU * 4.] {
                        for clockwise in [false, true] {
                            let source = motion(plane, radius, pitch, sweep, clockwise);
                            let limits = allocate(machine, source, requested())?;
                            assert!(
                                limits.maximum_velocity_mm_s <= 30. && limits.velocity_mm_s <= 5.
                            );
                            for fraction in [0., 0.25, 0.75, 1.] {
                                for a_sign in [-1., 0., 1.] {
                                    for j_sign in [-1., 0., 1.] {
                                        for step in 0..8 {
                                            let (a, j) = cartesian_derivatives(
                                                radius,
                                                pitch,
                                                f64::from(step) * std::f64::consts::FRAC_PI_4,
                                                if clockwise { -1. } else { 1. },
                                                fraction * limits.maximum_velocity_mm_s,
                                                a_sign * limits.acceleration_mm_s2,
                                                j_sign * limits.jerk_mm_s3,
                                            );
                                            for (component, axis) in
                                                [u, v, n].into_iter().enumerate()
                                            {
                                                assert!(a[component].abs() <= machine.axes[axis].acceleration_mm_s2,
                                                    "axis acceleration {plane:?} R={radius} pitch={pitch} A={a:?} limits={limits:?}");
                                                assert!(j[component].abs() <= machine.axes[axis].jerk_mm_s3,
                                                    "axis jerk {plane:?} R={radius} pitch={pitch} J={j:?} limits={limits:?}");
                                            }
                                            assert!(
                                                a[0].hypot(a[1]).hypot(a[2])
                                                    <= machine.trajectory.acceleration_mm_s2
                                            );
                                            assert!(
                                                j[0].hypot(j[1]).hypot(j[2])
                                                    <= machine.trajectory.jerk_mm_s3
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn absent_lathe_axis_is_ignored_but_helix_axial_jerk_is_not() -> Result<()> {
    let mut m = machine(false);
    m.axis_mask = 5;
    m.axes[1] = super::super::AxisDynamics {
        velocity_mm_s: 0.,
        acceleration_mm_s2: 0.,
        jerk_mm_s3: 0.,
    };
    let flat = motion(Plane::Xz, 1., 0., std::f64::consts::TAU, true);
    let a = allocate(m, flat, requested())?;
    assert_eq!(a, allocate(machine(false), flat, requested())?);
    let steep = motion(Plane::Xy, 1., 5., std::f64::consts::TAU, false);
    let mut low = machine(false);
    low.axes[2].jerk_mm_s3 = 0.01;
    let limited = allocate(low, steep, requested())?;
    assert!(limited.jerk_mm_s3 * 5. / 26_f64.sqrt() <= 0.01);
    assert!(limited.jerk_mm_s3 < allocate(machine(false), steep, requested())?.jerk_mm_s3);
    Ok(())
}

#[test]
fn feed_law_and_geometry_do_not_change_the_physical_box() -> Result<()> {
    let mut source = motion(Plane::Xz, 2., 0., std::f64::consts::TAU, false);
    let original = source;
    let a = allocate(machine(false), source, requested())?;
    source.feed = Feed::PerRevolution {
        mm_per_rev: 0.5,
        spindle: 0,
    };
    let b = allocate(machine(false), source, requested())?;
    assert_eq!(a, b);
    source.feed = original.feed;
    assert_eq!(source, original);
    Ok(())
}

#[test]
fn allocation_changes_with_available_travel_and_rejects_unrepresentable_geometry() -> Result<()> {
    let short = allocate(
        machine(false),
        motion(Plane::Xy, 1., 0., 0.01, false),
        requested(),
    )?;
    let long = allocate(
        machine(false),
        motion(Plane::Xy, 1., 0., 100., false),
        requested(),
    )?;
    assert!(short.maximum_velocity_mm_s < long.maximum_velocity_mm_s);
    assert!(
        short.acceleration_mm_s2 > long.acceleration_mm_s2 || short.jerk_mm_s3 > long.jerk_mm_s3
    );
    for radius in [0., f64::NAN, f64::INFINITY, 1e-300] {
        assert!(allocate(
            machine(false),
            motion(Plane::Xy, radius, 0., 1., false),
            requested()
        )
        .is_err());
    }
    Ok(())
}
