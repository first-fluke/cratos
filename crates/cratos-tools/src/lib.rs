//! Cratos Tools - Tool Registry and Execution Engine
//!
//! This crate provides the tool system for Cratos:
//! - Registry: Tool registration and discovery
//! - Runner: Tool execution engine with sandboxing
//! - Builtins: Built-in tools (file, http, git, etc.)
//! - Sandbox: Docker-based tool isolation
//! - MCP: Model Context Protocol client for external tools

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod browser;
pub mod builtins;
pub mod doctor;
pub mod error;
pub mod mcp;
pub mod registry;
pub mod runner;
pub mod sandbox;

pub use builtins::{register_builtins, register_builtins_with_config, BuiltinsConfig, ExecConfig, ExecMode};
pub use doctor::{ChecklistItem, Diagnosis, FailureCategory, ProbableCause, ToolDoctor};
pub use error::{Error, Result};
pub use registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolRegistry, ToolResult};
pub use runner::{ExecutionOptions, ExecutionResult, RunnerConfig, ToolRunner};
pub use sandbox::{
    DockerSandbox, Mount, NetworkMode, ResourceLimits, SandboxConfig, SandboxOutput, SandboxPolicy,
    ToolSandbox,
};

// Re-export MCP types
pub use mcp::{McpClient, McpClientConfig, McpError, McpServerConfig, McpTool, McpTransport};

// Re-export browser types
pub use browser::{BrowserAction, BrowserConfig, BrowserEngine, BrowserTool};
