//! Independent non-real-time geometry checks over the version-pinned contract.
use crate::{Diagnostic, Result};
use motion_command::{v2::Geometry, Plane, PointMm, Rotation};
use serde::Serialize;
use std::f64::consts::{FRAC_PI_2, TAU};

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Metrics {
    pub length_mm: f64,
    pub minimum_mm: [f64; 3],
    pub maximum_mm: [f64; 3],
    /// Numeric discrepancy at the supplied endpoint; never a fitting allowance.
    pub endpoint_discrepancy_mm: f64,
}
pub fn coordinates(p: PointMm) -> [f64; 3] {
    [p.x, p.y, p.z]
}
pub fn point(p: [f64; 3]) -> PointMm {
    PointMm {
        x: p[0],
        y: p[1],
        z: p[2],
    }
}
/// Positive-normal right-handed basis, independent of the writer's frame table.
pub fn basis(plane: Plane) -> (usize, usize, usize) {
    match plane {
        Plane::Xy => (0, 1, 2),
        Plane::Xz => (2, 0, 1),
        Plane::Yz => (1, 2, 0),
    }
}
fn problem(message: &str) -> Diagnostic {
    Diagnostic::new("geometry", "GEOMETRY", message)
}
pub fn validate(geometry: Geometry) -> Result<Metrics> {
    validate_with_floor(geometry, 1e-7)
}
// The source profile specifies its numeric consistency floor in source units.
// Callers convert it to millimetres; this is never an extra fit/blend allowance.
/// Check geometry using the source profile's dimensional numeric consistency
/// floor. This is not a fitting/blending allowance or a physical-error budget.
pub fn validate_with_floor(geometry: Geometry, numeric_floor_mm: f64) -> Result<Metrics> {
    if !numeric_floor_mm.is_finite() || numeric_floor_mm <= 0.0 {
        return Err(problem("Invalid numeric consistency floor"));
    }
    let (start, end) = match geometry {
        Geometry::Line { start, end } | Geometry::Circular { start, end, .. } => {
            (coordinates(start), coordinates(end))
        }
    };
    if !start.into_iter().chain(end).all(f64::is_finite) {
        return Err(problem("Nonfinite endpoint"));
    }
    let mut out = Metrics {
        length_mm: 0.0,
        minimum_mm: std::array::from_fn(|i| start[i].min(end[i])),
        maximum_mm: std::array::from_fn(|i| start[i].max(end[i])),
        endpoint_discrepancy_mm: 0.0,
    };
    match geometry {
        Geometry::Line { .. } => {
            out.length_mm = (end[0] - start[0])
                .hypot(end[1] - start[1])
                .hypot(end[2] - start[2])
        }
        Geometry::Circular {
            center,
            plane,
            rotation,
            sweep_radians: sweep,
            axial_rise_mm: rise,
            ..
        } => {
            let center = coordinates(center);
            let (u, v, n) = basis(plane);
            if !center.into_iter().all(f64::is_finite)
                || !sweep.is_finite()
                || sweep <= 0.0
                || sweep > 10_000.0 * TAU
                || !rise.is_finite()
            {
                return Err(problem("Invalid circular scalar or turn resource bound"));
            }
            if center[n] != start[n] || end[n] - start[n] != rise {
                return Err(problem("Inconsistent axis point or signed axial rise"));
            }
            let a = [start[u] - center[u], start[v] - center[v]];
            let b = [end[u] - center[u], end[v] - center[v]];
            let radius = a[0].hypot(a[1]);
            let other = b[0].hypot(b[1]);
            if !radius.is_finite() || radius <= 0.0 {
                return Err(problem("Invalid circle radius"));
            }
            let allowance = numeric_floor_mm.max(radius * 1e-6);
            if (radius - other).abs() > allowance {
                return Err(problem("Circular endpoint radii disagree"));
            }
            let direction = if rotation == Rotation::Clockwise {
                -1.0
            } else {
                1.0
            };
            let angle = a[1].atan2(a[0]);
            let relative = (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1]);
            let residual = relative - direction * sweep;
            if residual.sin().atan2(residual.cos()).abs() * radius > allowance {
                return Err(problem(
                    "Circular sweep differs from supplied endpoint angle",
                ));
            }
            let finish = angle + direction * sweep;
            let discrepancy = (center[u] + radius * finish.cos() - end[u])
                .hypot(center[v] + radius * finish.sin() - end[v]);
            if discrepancy > allowance * 2.0_f64.sqrt() {
                return Err(problem(
                    "Circular sweep/direction does not reach supplied endpoint",
                ));
            }
            out.endpoint_discrepancy_mm = discrepancy.max((radius - other).abs());
            out.length_mm = (radius * sweep).hypot(rise);
            for i in 0..4 {
                let cardinal = i as f64 * FRAC_PI_2;
                let along = (direction * (cardinal - angle)).rem_euclid(TAU);
                if along <= sweep {
                    // Cardinal values avoid a sin(pi) rounding artifact in bounds.
                    let du = [1.0, 0.0, -1.0, 0.0][i];
                    let dv = [0.0, 1.0, 0.0, -1.0][i];
                    for (axis, value) in
                        [(u, center[u] + radius * du), (v, center[v] + radius * dv)]
                    {
                        out.minimum_mm[axis] = out.minimum_mm[axis].min(value);
                        out.maximum_mm[axis] = out.maximum_mm[axis].max(value);
                    }
                }
            }
        }
    }
    if !out.length_mm.is_finite()
        || !out
            .minimum_mm
            .into_iter()
            .chain(out.maximum_mm)
            .all(f64::is_finite)
    {
        return Err(problem("Geometry calculation overflow"));
    }
    Ok(out)
}
