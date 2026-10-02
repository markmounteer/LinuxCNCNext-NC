//! Spindle-zero capability evidence supplied by the pinned controller host.
//! This is a preparation contract, never permission to enable a machine.

/// Hash a bounded, unambiguously encoded observation from the trusted host.
/// Hashing does not authenticate that observation: the host must first verify
/// the live HAL graph, runtime instance and policy. Never call this on job data.
pub fn witness_identity(witness: &[u8]) -> Result<[u8; 32], &'static str> {
    use sha2::{Digest, Sha256};
    if witness.is_empty() || witness.len() > 16_384 {
        return Err("spindle host witness must contain 1..16384 bytes");
    }
    let mut hash = Sha256::new();
    hash.update(b"nextnc-spindle-host-witness-v1\0");
    hash.update((witness.len() as u64).to_le_bytes());
    hash.update(witness);
    Ok(hash.finalize().into())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeedbackPolicy {
    pub maximum_rps: f64,
    pub heartbeat_timeout_s: f64,
    pub comparison_window_s: f64,
    pub position_error_revs: f64,
    pub relative_error: f64,
}

impl FeedbackPolicy {
    pub fn valid(self) -> bool {
        [
            self.maximum_rps,
            self.heartbeat_timeout_s,
            self.comparison_window_s,
            self.position_error_revs,
        ]
        .into_iter()
        .all(|v| v.is_finite() && v > 0.0)
            && self.relative_error.is_finite()
            && (0.0..1.0).contains(&self.relative_error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capability {
    /// Host-observed identity of the compatible task/motmod/shim, authoritative
    /// feedback producer, connected channels and their execution order. An INI
    /// capability bit or user-provided digest alone cannot establish this.
    pub identity: [u8; 32],
    pub feedback: FeedbackPolicy,
    pub css: bool,
}

impl Capability {
    pub fn valid(self) -> bool {
        self.identity != [0; 32] && self.feedback.valid()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CssDemand {
    pub surface_mm_s: f64,
    pub maximum_rpm: f64,
    pub clockwise: bool,
    /// Work + G92 + tool X, in physical radius mm, never diameter display units.
    pub x_offset_mm: f64,
}

impl CssDemand {
    /// Pinned motmod computes RPM = factor / abs(commanded X - X offset),
    /// then applies the spindle override and RPM cap. The host only converts
    /// length units. This demand must never substitute for measured feedback.
    pub fn factor_rpm_mm(self) -> f64 {
        self.surface_mm_s * (60.0 / std::f64::consts::TAU)
    }
    pub fn valid(self) -> bool {
        self.surface_mm_s.is_finite()
            && self.surface_mm_s > 0.0
            && self.maximum_rpm.is_finite()
            && self.maximum_rpm > 0.0
            && self.x_offset_mm.is_finite()
            && self.factor_rpm_mm().is_finite()
    }
}
