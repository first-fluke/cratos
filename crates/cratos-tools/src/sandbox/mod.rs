//! Container Sandbox - Tool isolation using container runtimes
//!
//! This module provides secure sandboxing for tool execution using container runtimes.
//! Supports multiple runtimes:
//! - Docker: Cross-platform, process-based isolation
//! - Apple Container: macOS 26+ native, VM-based isolation (stronger security)
//!
//! Key features:
//! - Network isolation (default: none)
//! - Read-only filesystem mounts
//! - Resource limits (CPU, memory)
//! - Tool-specific isolation policies
//! - Automatic runtime detection

#![forbid(unsafe_code)]

mod config;
mod docker;
mod limits;
mod mount;
mod output;
mod policy;
mod runtime;
mod unified;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use config::SandboxConfig;
pub use docker::DockerSandbox;
pub use limits::ResourceLimits;
pub use mount::Mount;
pub use output::SandboxOutput;
pub use policy::{NetworkMode, SandboxPolicy};
pub use runtime::ContainerRuntime;
pub use unified::{ToolSandbox, UnifiedSandbox};
