use nextnc_task::shaping::{Allowance, Geometry, Kernel, Term};
use std::error::Error;
type TestResult = Result<(), Box<dyn Error>>;

fn kernel() -> Result<Kernel, Box<dyn Error>> {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/common-xy-kernel.json"))?;
    let terms: Vec<(u32, f64)> = serde_json::from_value(value["terms"].clone())?;
    Ok(Kernel::from_terms(
        1_000_000,
        &terms
            .into_iter()
            .map(|(delay_ticks, weight)| Term {
                delay_ticks,
                weight,
            })
            .collect::<Vec<_>>(),
    )?)
}
fn ledger() -> Allowance {
    Allowance {
        source_corridor_mm: 0.002,
        upstream_error_mm: 0.0,
        arithmetic_error_mm: 1e-9,
        maximum_position_norm_mm: 20.0,
    }
}
fn circle() -> Geometry {
    Geometry {
        curvature_per_mm: 1.0,
        tangent_jump_sum: 0.0,
    }
}

/// Independently evaluate the pinned cubic from endpoint values/derivatives,
/// rather than using the allocator's delay-variance formula or B-spline basis.
fn output<F: Fn(f64) -> [f64; 2]>(kernel: &Kernel, tick: i32, fraction: f64, raw: F) -> [f64; 2] {
    let dt = f64::from(kernel.period_ns()) * 1e-9;
    let history: [[f64; 2]; 4] = std::array::from_fn(|i| {
        let mut point = [0.0; 2];
        for term in kernel.terms() {
            let sample = raw((f64::from(tick) - 3.0 + i as f64 - f64::from(term.delay_ticks)) * dt);
            for a in 0..2 {
                point[a] += term.weight * sample[a];
            }
        }
        point
    });
    std::array::from_fn(|axis| {
        let [a, b, c, d] = history.map(|p| p[axis]);
        let p0 = (a + 4.0 * b + c) / 6.0;
        let p1 = (b + 4.0 * c + d) / 6.0;
        let v0 = (c - a) / 2.0;
        let v1 = (d - b) / 2.0;
        let t = fraction;
        (2.0 * t.powi(3) - 3.0 * t * t + 1.0) * p0
            + (t.powi(3) - 2.0 * t * t + t) * v0
            + (-2.0 * t.powi(3) + 3.0 * t * t) * p1
            + (t.powi(3) - t * t) * v1
    })
}

/// Unit-rate parameter with acceleration, cruise, stop, hold and reversal.
/// This test does not need a constant angular speed assumption.
fn travel(t: f64) -> f64 {
    if t <= 0.0 {
        0.0
    } else if t < 0.2 {
        2.5 * t * t
    } else if t < 0.8 {
        t - 0.1
    } else if t < 1.0 {
        let u = t - 0.8;
        0.7 + u - 2.5 * u * u
    } else if t < 1.2 {
        0.8
    } else if t < 1.4 {
        let u = t - 1.2;
        0.8 - 2.5 * u * u
    } else {
        0.7 - (t - 1.4)
    }
}

#[test]
fn captured_kernel_exposes_old_arc_error_and_allocates_within_unchanged_allowance() -> TestResult {
    let k = kernel()?;
    let c = k.allocate(circle(), ledger(), 30.0)?;
    assert!(c.maximum_velocity_mm_s > 1.8 && c.maximum_velocity_mm_s < 2.0);
    assert!(k.geometric_error_upper(circle(), 5.0)? > 0.013);
    assert!(c.total_error_upper_mm <= ledger().source_corridor_mm);
    let mut maxima = [0.0_f64; 2];
    for (lane, speed) in [5.0, c.maximum_velocity_mm_s].into_iter().enumerate() {
        for tick in (0..2600).step_by(3) {
            for fraction in [0.0, 0.125, 0.5, 0.875, 1.0] {
                let p = output(&k, tick, fraction, |t| {
                    let angle = speed * travel(t);
                    [5.0 + angle.cos(), 3.0 + angle.sin()]
                });
                maxima[lane] = maxima[lane].max(((p[0] - 5.0).hypot(p[1] - 3.0) - 1.0).abs());
            }
        }
    }
    assert!(maxima[0] > 0.013);
    assert!(maxima[1] <= c.total_error_upper_mm);
    Ok(())
}

#[test]
fn sudden_speed_changes_and_repeated_direction_changes_remain_covered() -> TestResult {
    let k = kernel()?;
    let c = k.allocate(circle(), ledger(), 30.0)?;
    for tick in (0..2000).step_by(5) {
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            // Triangle-wave arclength is Lipschitz with speed <= the cap,
            // despite instantaneous reversals. A constant-speed formula alone
            // cannot justify this input; the pairwise variance argument can.
            let p = output(&k, tick, fraction, |t| {
                let s = (t.rem_euclid(0.6) - 0.3).abs() * c.maximum_velocity_mm_s;
                [s.cos(), s.sin()]
            });
            assert!((p[0].hypot(p[1]) - 1.0).abs() <= c.total_error_upper_mm);
        }
    }
    Ok(())
}

#[test]
fn corner_atom_is_required_and_bounds_actual_corner_rounding() -> TestResult {
    let k = kernel()?;
    let geometry = Geometry {
        curvature_per_mm: 0.0,
        tangent_jump_sum: 2.0_f64.sqrt().next_up(),
    };
    let c = k.allocate(geometry, ledger(), 5.0)?;
    let mut with_jump = 0.0_f64;
    let mut ignored_jump = 0.0_f64;
    for tick in 800..1300 {
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for (speed, maximum) in [
                (c.maximum_velocity_mm_s, &mut with_jump),
                (5.0, &mut ignored_jump),
            ] {
                let p = output(&k, tick, fraction, |t| {
                    let s = speed * (t - 1.0);
                    [s.min(0.0), s.max(0.0)]
                });
                *maximum = maximum.max(p[0].abs().min(p[1].abs()));
            }
        }
    }
    assert!(ignored_jump > 0.01);
    assert!(with_jump > 0.0 && with_jump <= c.total_error_upper_mm);
    Ok(())
}

#[test]
fn straight_motion_retains_machine_ceiling_and_delays_do_not_invent_curvature() -> TestResult {
    let geometry = Geometry {
        curvature_per_mm: 0.0,
        tangent_jump_sum: 0.0,
    };
    let k = kernel()?;
    assert_eq!(
        k.allocate(geometry, ledger(), 30.0)?.maximum_velocity_mm_s,
        30.0
    );
    let a = Kernel::from_terms(
        1_000_000,
        &[Term {
            delay_ticks: 0,
            weight: 1.0,
        }],
    )?;
    let b = Kernel::from_terms(
        1_000_000,
        &[Term {
            delay_ticks: 200,
            weight: 1.0,
        }],
    )?;
    assert_eq!(a.variance_upper_s2(), b.variance_upper_s2());
    assert_ne!(a.identity(), b.identity());
    assert_eq!(b.allocate(circle(), ledger(), 30.0)?.history_ticks, 203);
    Ok(())
}

#[test]
fn kernel_identity_binds_exact_coefficients_and_period_without_renormalizing() -> TestResult {
    let k = kernel()?;
    let mut terms = k.terms().to_vec();
    let original = terms.clone();
    terms[0].weight = terms[0].weight.next_up();
    let changed = Kernel::from_terms(k.period_ns(), &terms)?;
    assert_ne!(changed.identity(), k.identity());
    assert_eq!(changed.terms(), terms);
    assert_ne!(
        Kernel::from_terms(2_000_000, &original)?.identity(),
        k.identity()
    );
    assert!(
        Kernel::from_terms(2_000_000, &original)?.variance_upper_s2()
            >= 4.0 * k.variance_upper_s2()
    );
    Ok(())
}

#[test]
fn exhausted_or_unknown_error_ledger_is_refused() -> TestResult {
    let k = kernel()?;
    for bad in [
        Allowance {
            arithmetic_error_mm: 0.002,
            ..ledger()
        },
        Allowance {
            upstream_error_mm: 0.002,
            ..ledger()
        },
        Allowance {
            source_corridor_mm: 0.0,
            ..ledger()
        },
        Allowance {
            maximum_position_norm_mm: f64::INFINITY,
            ..ledger()
        },
    ] {
        assert!(k.allocate(circle(), bad, 30.0).is_err());
    }
    for bad in [
        Geometry {
            curvature_per_mm: f64::NAN,
            tangent_jump_sum: 0.0,
        },
        Geometry {
            curvature_per_mm: 1.0,
            tangent_jump_sum: -1.0,
        },
    ] {
        assert!(k.allocate(bad, ledger(), 30.0).is_err());
    }
    let mass = Kernel::from_terms(
        1_000_000,
        &[Term {
            delay_ticks: 0,
            weight: 1.0 + 1e-13,
        }],
    )?;
    assert!(mass
        .allocate(
            circle(),
            Allowance {
                maximum_position_norm_mm: 1e12,
                ..ledger()
            },
            30.0
        )
        .is_err());
    Ok(())
}

#[test]
fn invalid_or_unbounded_kernels_are_refused() {
    for terms in [
        vec![],
        vec![Term {
            delay_ticks: 0,
            weight: 0.0,
        }],
        vec![Term {
            delay_ticks: 256,
            weight: 1.0,
        }],
        vec![Term {
            delay_ticks: 0,
            weight: f64::NAN,
        }],
        vec![Term {
            delay_ticks: 0,
            weight: 0.9,
        }],
        vec![
            Term {
                delay_ticks: 1,
                weight: 0.5,
            },
            Term {
                delay_ticks: 1,
                weight: 0.5,
            },
        ],
        vec![
            Term {
                delay_ticks: 2,
                weight: 0.5,
            },
            Term {
                delay_ticks: 1,
                weight: 0.5,
            },
        ],
        (0..33)
            .map(|delay_ticks| Term {
                delay_ticks,
                weight: 1.0 / 33.0,
            })
            .collect(),
    ] {
        assert!(Kernel::from_terms(1_000_000, &terms).is_err());
    }
    assert!(Kernel::from_terms(
        0,
        &[Term {
            delay_ticks: 0,
            weight: 1.0
        }]
    )
    .is_err());
}

#[test]
fn combined_curvature_and_corner_terms_share_one_budget() -> TestResult {
    let k = kernel()?;
    let smooth = k.allocate(circle(), ledger(), 30.0)?;
    let combined = k.allocate(
        Geometry {
            tangent_jump_sum: 0.1,
            ..circle()
        },
        ledger(),
        30.0,
    )?;
    assert!(combined.maximum_velocity_mm_s < smooth.maximum_velocity_mm_s);
    assert!(combined.geometric_error_upper_mm <= ledger().source_corridor_mm);
    assert!(combined.total_error_upper_mm <= ledger().source_corridor_mm);
    let spent = k.allocate(
        circle(),
        Allowance {
            upstream_error_mm: 0.001,
            ..ledger()
        },
        30.0,
    )?;
    assert!(spent.maximum_velocity_mm_s < smooth.maximum_velocity_mm_s);
    Ok(())
}
