//! Spindle-zero capability evidence supplied by the pinned controller host.
//! This is a preparation contract, never permission to enable a machine.

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
