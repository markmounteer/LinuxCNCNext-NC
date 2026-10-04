//! Offline synthetic component evidence, never machine execution or permission.
use nextnc_task::shaping::{Allowance, Geometry, Kernel, Term};
use serde_json::json;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/common-xy-kernel.json"))?;
    let raw: Vec<(u32, f64)> = serde_json::from_value(fixture["terms"].clone())?;
    let terms: Vec<_> = raw
        .iter()
        .map(|&(delay_ticks, weight)| Term {
            delay_ticks,
            weight,
        })
        .collect();
    let mut cases = Vec::new();
    for period in [100_000, 1_000_000, 10_000_000] {
        let kernel = Kernel::from_terms(period, &terms)?;
        for curvature in [0.0, 0.001, 1.0, 100.0] {
            for jumps in [0.0, 0.1, 2.0_f64.sqrt().next_up(), 2.0] {
                for physical in [0.000001, 30.0, 1000.0] {
                    let geometry = Geometry {
                        curvature_per_mm: curvature,
                        tangent_jump_sum: jumps,
                    };
                    let allowance = Allowance {
                        source_corridor_mm: 0.002,
                        upstream_error_mm: 0.0005,
                        arithmetic_error_mm: 1e-9,
                        maximum_position_norm_mm: 20.0,
                    };
                    let c = kernel.allocate(geometry, allowance, physical)?;
                    cases.push(json!({"period_ns":period,"kernel_identity":c.kernel_identity,
                        "history_ticks":c.history_ticks,"curvature_per_mm":curvature,"tangent_jump_sum":jumps,
                        "physical_maximum_velocity_mm_s":physical,"source_corridor_mm":allowance.source_corridor_mm,
                        "upstream_error_mm":allowance.upstream_error_mm,"arithmetic_error_mm":allowance.arithmetic_error_mm,
                        "maximum_position_norm_mm":allowance.maximum_position_norm_mm,
                        "variance_upper_s2":c.variance_upper_s2,"coefficient_mass_error_mm":c.coefficient_mass_error_mm,
                        "maximum_velocity_mm_s":c.maximum_velocity_mm_s,
                        "geometric_error_upper_mm":c.geometric_error_upper_mm,"total_error_upper_mm":c.total_error_upper_mm}));
                }
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"schema":"nextnc-stage5/shaping-budget-component/1",
        "fixture":fixture,"cases":cases,"stage5Accepted":false,"liveAdmissionIntegrated":false,
        "scope":"Synthetic Rust component allocations; no runtime path or hardware certificate."}))?
    );
    Ok(())
}
