//! Orchestrator - Main execution loop
//!
//! This module provides the main orchestration logic that ties together
//! the planner, tools, memory, and replay systems.
//!
//! # Module Structure
//!
//! - `types`: Core types (SkillMatch, ExecutionResult, etc.)
//! - `config`: Configuration types (OrchestratorConfig, OrchestratorInput)
//! - `core`: Orchestrator struct and builder methods
//! - `process`: Main execution loop
//! - `tool_execution`: Tool execution logic
//! - `planning`: Planning methods (dispatch_plan, plan_with_fallback)
//! - `multi_persona`: Multi-persona execution modes
//! - `helpers`: Utility methods (emit, log_event)
//! - `sanitize`: Sanitization and validation helpers

mod config;
mod core;
mod helpers;
mod multi_persona;
mod planning;
mod post_execution;
mod process;
mod result_builder;
mod routing;
mod sanitize;
mod session_context;
mod tool_execution;
mod types;

#[cfg(test)]
mod tests;

// Re-export public types
pub use config::{OrchestratorConfig, OrchestratorInput};
pub use core::Orchestrator;
pub use types::{
    ExecutionArtifact, ExecutionResult, ExecutionStatus, SkillMatch, SkillRouting, ToolCallRecord,
};
