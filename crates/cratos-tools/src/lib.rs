//! Cratos Tools - Tool Registry and Execution Engine
//!
//! This crate provides the tool system for Cratos:
//! - Registry: Tool registration and discovery
//! - Runner: Tool execution engine with sandboxing
//! - Builtins: Built-in tools (file, http, git, etc.)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod builtins;
pub mod doctor;
pub mod error;
pub mod registry;
pub mod runner;

pub use builtins::register_builtins;
pub use doctor::{ChecklistItem, Diagnosis, FailureCategory, ProbableCause, ToolDoctor};
pub use error::{Error, Result};
pub use registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolRegistry, ToolResult};
pub use runner::{ExecutionOptions, ExecutionResult, RunnerConfig, ToolRunner};
