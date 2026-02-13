//! Tool Doctor - Diagnostic and troubleshooting system
//!
//! This module provides self-diagnosis capabilities for tool failures,
//! generating cause analysis and resolution checklists.

mod category;
mod core;
mod formatter;
mod handlers;
mod patterns;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use category::FailureCategory;
pub use core::ToolDoctor;
pub use types::{Alternative, ChecklistItem, Diagnosis, ProbableCause};
