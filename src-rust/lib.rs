//! Non-real-time preparation only. No controller, HAL, socket or motion API.
pub mod diagnostic;
pub mod geometry;
pub mod json;
pub mod part21;
pub mod profile_graph;
pub mod shape;
pub mod units;
pub use diagnostic::{Diagnostic, Result};
pub use motion_command as contract;
