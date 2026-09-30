//! Non-real-time preparation only. No controller, HAL, socket or motion API.
pub mod capabilities;
pub mod command_audit;
pub mod compiled;
pub mod diagnostic;
pub mod geometry;
pub mod json;
pub mod part21;
pub mod plan;
pub mod profile;
pub mod profile_graph;
pub mod shape;
pub mod tool_table;
pub mod units;
pub use diagnostic::{Diagnostic, Result};
pub use motion_command as contract;
