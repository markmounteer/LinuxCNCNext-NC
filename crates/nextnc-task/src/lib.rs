//! Task-owned native execution machinery. No direct planner or HAL access.
//!
//! This module is an integration component, not an execution permission API.
//! Host evidence must come from the pinned task/motion/I/O boundary. The checked
//! C ABI, coordinate binding and full-stack qualification are separate gates.
#![forbid(unsafe_code)]

pub mod binding;
pub mod lifecycle;
pub mod lowering;
pub mod receipts;
pub mod steps;
