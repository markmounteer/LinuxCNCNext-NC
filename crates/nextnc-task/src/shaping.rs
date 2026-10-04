//! Off-thread geometric error budget for a common positive FIR followed by the
//! pinned rate-one cubic. This is a preparation component, not live authority.
//!
//! For a unit-speed source curve with curvature bounded by K and total tangent
//! jumps J in the retained history window, a positive weighted average lies
//! within K Var(S)/2 + J sqrt(Var(S))/2 of the source at mean arclength. If all
//! retained raw motion has speed <= V, Var(S) <= V^2 Var(T), including arbitrary
//! changes of speed, holds and rest-clamped endpoints. The cubic adds dt^2/3 to
//! the normalized FIR delay variance. Coefficient mass error is reserved
//! separately; runtime coefficients are never normalized or changed here.
//!
//! Callers must establish the actual immutable kernel, rate-one interpolation,
//! whole retained-history speed/geometry bounds, upstream geometric error and
//! arithmetic reserve. This module does not establish those execution facts,
//! allow an unknown kernel, certify physical tracking, or authorize a machine.
use sha2::{Digest, Sha256};

/// Match the current coordinated FIR's bounded sparse kernel, not an arbitrary
/// unbounded filter description supplied by a job.
pub const MAX_TERMS: usize = 32;
pub const HISTORY_SAMPLES: u32 = 256;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Term {
    pub delay_ticks: u32,
    pub weight: f64,
}

#[derive(Clone, Debug)]
pub struct Kernel {
    terms: Vec<Term>,
    period_ns: u32,
    identity: [u8; 32],
    mass_error_upper: f64,
    variance_upper_s2: f64,
}

/// Bounds for the *entire* source portion touched by retained FIR/cubic history.
/// Curvature is measured per unit arclength; tangent jumps are vector norms,
/// not angles. Unknown primitive boundaries must not be assigned zero jump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub curvature_per_mm: f64,
    pub tangent_jump_sum: f64,
}

/// All terms refer to distance from the reviewed source toolpath, not CAD error.
/// No allowance is inferred from Fusion metadata or an enabled filter alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Allowance {
    pub source_corridor_mm: f64,
    pub upstream_error_mm: f64,
    pub arithmetic_error_mm: f64,
    /// Norm of all raw coordinates in the same frame used by the FIR.
    pub maximum_position_norm_mm: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Certificate {
    pub kernel_identity: [u8; 32],
    pub period_ns: u32,
    pub history_ticks: u32,
    pub geometry: Geometry,
    pub allowance: Allowance,
    pub variance_upper_s2: f64,
    pub coefficient_mass_error_mm: f64,
    pub maximum_velocity_mm_s: f64,
    pub geometric_error_upper_mm: f64,
    pub total_error_upper_mm: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error(pub &'static str);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

fn finite_nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

// Each operation rounds a nonnegative bound in the indicated direction.
// Preserve exact zero so a straight source does not acquire invented curvature.
fn add_up(a: f64, b: f64) -> f64 {
    if a == 0.0 {
        b
    } else if b == 0.0 {
        a
    } else {
        (a + b).next_up()
    }
}
fn mul_up(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        0.0
    } else {
        (a * b).next_up()
    }
}
fn down_nonnegative(value: f64) -> f64 {
    value.next_down().max(0.0)
}

impl Kernel {
    /// Freeze actual positive coefficients, ordered unique delays and period.
    /// The digest covers exact binary64 coefficients and never stands in for
    /// host verification of which kernel is executing.
    pub fn from_terms(period_ns: u32, terms: &[Term]) -> Result<Self> {
        if period_ns == 0 || terms.is_empty() || terms.len() > MAX_TERMS {
            return Err(Error("missing or oversized shaping kernel/period"));
        }
        let (mut lower, mut upper, mut last) = (0.0, 0.0, None);
        let mut hash = Sha256::new();
        hash.update(b"nextnc-common-positive-fir-rate-one-cubic-v1\0");
        hash.update(period_ns.to_le_bytes());
        hash.update((terms.len() as u32).to_le_bytes());
        for term in terms {
            if !term.weight.is_finite()
                || term.weight <= 0.0
                || term.delay_ticks >= HISTORY_SAMPLES
                || last.is_some_and(|previous| term.delay_ticks <= previous)
            {
                return Err(Error("invalid shaping coefficient or delay"));
            }
            lower = down_nonnegative(lower + term.weight);
            upper = add_up(upper, term.weight);
            last = Some(term.delay_ticks);
            hash.update(term.delay_ticks.to_le_bytes());
            hash.update(term.weight.to_bits().to_le_bytes());
        }
        let mass_error_upper = (1.0 - lower).abs().max((upper - 1.0).abs()).next_up();
        if lower <= 0.0 || !upper.is_finite() || mass_error_upper > 1e-12 {
            return Err(Error("shaping coefficients do not establish unit mass"));
        }
        // Pairwise variance avoids subtracting two nearly equal large moments:
        // Var(T) = sum(i<j) wi*wj*(ti-tj)^2 / (sum wi)^2.
        let mut pair_sum = 0.0;
        for (i, left) in terms.iter().enumerate() {
            for right in &terms[i + 1..] {
                let delta = f64::from(right.delay_ticks - left.delay_ticks);
                pair_sum = add_up(
                    pair_sum,
                    mul_up(mul_up(left.weight, right.weight), delta * delta),
                );
            }
        }
        let mass_squared_lower = down_nonnegative(lower * lower);
        let variance_ticks = if pair_sum == 0.0 {
            0.0
        } else {
            (pair_sum / mass_squared_lower).next_up()
        };
        // Uniform cubic B-spline basis is nonnegative with unit mass and delay
        // variance 1/3 at every fractional time, including between servo knots.
        let with_cubic = add_up(variance_ticks, (1.0_f64 / 3.0).next_up());
        let dt_upper = (f64::from(period_ns) / 1e9).next_up();
        let variance_upper_s2 = mul_up(with_cubic, mul_up(dt_upper, dt_upper));
        if !variance_upper_s2.is_finite() || variance_upper_s2 <= 0.0 {
            return Err(Error("unrepresentable shaping delay variance"));
        }
        Ok(Self {
            terms: terms.to_vec(),
            period_ns,
            identity: hash.finalize().into(),
            mass_error_upper,
            variance_upper_s2,
        })
    }

    pub fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }
    pub fn period_ns(&self) -> u32 {
        self.period_ns
    }
    /// Last retained FIR delay plus the three preceding rate-one cubic knots.
    pub fn history_ticks(&self) -> u32 {
        self.terms.last().map_or(0, |t| t.delay_ticks) + 3
    }
    pub fn variance_upper_s2(&self) -> f64 {
        self.variance_upper_s2
    }

    /// Geometric contribution alone, with outward-rounded arithmetic. A speed
    /// bound must include old retained samples as well as newly admitted motion.
    pub fn geometric_error_upper(&self, geometry: Geometry, velocity: f64) -> Result<f64> {
        if !finite_nonnegative(geometry.curvature_per_mm)
            || !finite_nonnegative(geometry.tangent_jump_sum)
            || !finite_nonnegative(velocity)
        {
            return Err(Error("unknown shaping geometry or speed bound"));
        }
        let smooth = mul_up(
            0.5,
            mul_up(
                geometry.curvature_per_mm,
                mul_up(mul_up(velocity, velocity), self.variance_upper_s2),
            ),
        );
        let corners = mul_up(
            0.5,
            mul_up(
                geometry.tangent_jump_sum,
                mul_up(velocity, self.variance_upper_s2.sqrt().next_up()),
            ),
        );
        let result = add_up(smooth, corners);
        if !result.is_finite() {
            return Err(Error("unrepresentable shaping geometric error"));
        }
        Ok(result)
    }

    /// Allocate a hard scalar speed ceiling within an explicit remaining error
    /// ledger. This does not raise the machine ceiling or modify programmed feed.
    /// Fixed work: 64 bisections after at most 496 pair terms at construction.
    pub fn allocate(
        &self,
        geometry: Geometry,
        allowance: Allowance,
        physical_maximum_velocity_mm_s: f64,
    ) -> Result<Certificate> {
        if !allowance.source_corridor_mm.is_finite()
            || allowance.source_corridor_mm <= 0.0
            || ![
                allowance.upstream_error_mm,
                allowance.arithmetic_error_mm,
                allowance.maximum_position_norm_mm,
            ]
            .into_iter()
            .all(finite_nonnegative)
            || !physical_maximum_velocity_mm_s.is_finite()
            || physical_maximum_velocity_mm_s <= 0.0
        {
            return Err(Error("invalid shaping error ledger or machine ceiling"));
        }
        let mass = mul_up(self.mass_error_upper, allowance.maximum_position_norm_mm);
        let fixed = add_up(
            add_up(allowance.upstream_error_mm, allowance.arithmetic_error_mm),
            mass,
        );
        if !fixed.is_finite() || fixed >= allowance.source_corridor_mm {
            return Err(Error("shaping error reserve exhausts the source allowance"));
        }
        // Validate geometry even when the first physical ceiling would pass.
        self.geometric_error_upper(geometry, 0.0)?;
        let total = |v| {
            self.geometric_error_upper(geometry, v)
                .map(|e| add_up(fixed, e))
        };
        let ceiling_fits = total(physical_maximum_velocity_mm_s)
            .is_ok_and(|value| value <= allowance.source_corridor_mm);
        let velocity = if ceiling_fits {
            physical_maximum_velocity_mm_s
        } else {
            let (mut low, mut high) = (0.0, physical_maximum_velocity_mm_s);
            for _ in 0..64 {
                let mid = low + (high - low) * 0.5;
                if total(mid).is_ok_and(|value| value <= allowance.source_corridor_mm) {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            low
        };
        if velocity <= 0.0 {
            return Err(Error("no representable speed within shaping allowance"));
        }
        let geometric = self.geometric_error_upper(geometry, velocity)?;
        let total = add_up(fixed, geometric);
        if !total.is_finite() || total > allowance.source_corridor_mm {
            return Err(Error("shaping allocation did not certify"));
        }
        Ok(Certificate {
            kernel_identity: self.identity,
            period_ns: self.period_ns,
            history_ticks: self.history_ticks(),
            geometry,
            allowance,
            variance_upper_s2: self.variance_upper_s2,
            coefficient_mass_error_mm: mass,
            maximum_velocity_mm_s: velocity,
            geometric_error_upper_mm: geometric,
            total_error_upper_mm: total,
        })
    }
}
