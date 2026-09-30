use nextnc_native::{
    contract::{v2::Geometry, Plane, Rotation},
    geometry::{point, validate},
};
use std::f64::consts::{FRAC_PI_2, TAU};
#[test]
fn analytic_metrics_cover_planes_units_senses_turns_and_signed_rise(
) -> Result<(), Box<dyn std::error::Error>> {
    // Explicit independent axis mappings, not geometry::basis under test.
    for (plane, axes) in [
        (Plane::Xy, [0, 1, 2]),
        (Plane::Xz, [2, 0, 1]),
        (Plane::Yz, [1, 2, 0]),
    ] {
        for scale in [1.0, 25.4] {
            for clockwise in [false, true] {
                for sweep in [FRAC_PI_2, TAU, 2.5 * TAU] {
                    for rise in [-3.0, 0.0, 3.0] {
                        let [u, v, n] = axes;
                        let mut start = [0.0; 3];
                        let mut end = [0.0; 3];
                        let radius = 2.0 * scale;
                        start[u] = radius;
                        end[u] = radius * sweep.cos();
                        end[v] = radius * sweep.sin() * if clockwise { -1.0 } else { 1.0 };
                        end[n] = rise * scale;
                        let curve = Geometry::Circular {
                            start: point(start),
                            end: point(end),
                            center: point([0.0; 3]),
                            plane,
                            rotation: if clockwise {
                                Rotation::Clockwise
                            } else {
                                Rotation::Counterclockwise
                            },
                            sweep_radians: sweep,
                            axial_rise_mm: rise * scale,
                        };
                        let m = validate(curve)?;
                        assert!((m.length_mm - (radius * sweep).hypot(rise * scale)).abs() < 1e-10);
                        // Independently sample the continuous analytic path and require enclosure.
                        for i in 0..=1024 {
                            let t = i as f64 / 1024.0;
                            let mut p = [0.0; 3];
                            let a = sweep * t * if clockwise { -1.0 } else { 1.0 };
                            p[u] = radius * a.cos();
                            p[v] = radius * a.sin();
                            p[n] = rise * scale * t;
                            for (axis, value) in p.iter().enumerate() {
                                assert!(
                                    *value >= m.minimum_mm[axis] - 1e-10
                                        && *value <= m.maximum_mm[axis] + 1e-10
                                );
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
fn wrong_sweep_nonfinite_zero_radius_and_axial_contradictions_fail() {
    let curve = Geometry::Circular {
        start: point([1.0, 0.0, 0.0]),
        end: point([1.0, 0.0, -2.0]),
        center: point([0.0; 3]),
        plane: Plane::Xy,
        rotation: Rotation::Clockwise,
        sweep_radians: TAU,
        axial_rise_mm: -2.0,
    };
    let mut bad = curve;
    if let Geometry::Circular {
        ref mut sweep_radians,
        ..
    } = bad
    {
        *sweep_radians = FRAC_PI_2;
    }
    assert!(validate(bad).is_err());
    let mut bad = curve;
    if let Geometry::Circular {
        ref mut axial_rise_mm,
        ..
    } = bad
    {
        *axial_rise_mm = 2.0;
    }
    assert!(validate(bad).is_err());
    let mut bad = curve;
    if let Geometry::Circular { ref mut center, .. } = bad {
        center.x = 1.0;
    }
    assert!(validate(bad).is_err());
    let mut bad = curve;
    if let Geometry::Circular {
        ref mut sweep_radians,
        ..
    } = bad
    {
        *sweep_radians = f64::INFINITY;
    }
    assert!(validate(bad).is_err());
}
