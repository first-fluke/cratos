//! Bash tool — PTY-based shell execution with 5-layer security
//!
//! Unlike the `exec` tool which uses `Command::new()` (no shell, no pipes),
//! this tool spawns a real bash shell via PTY, supporting:
//! - Pipe chains: `ps aux | grep node | head -20`
//! - Redirections: `echo data > /tmp/out.txt`
//! - Command chaining: `cd /project && make clean && make`
//! - Background sessions with poll/send_keys/kill
//! - Real-time output streaming via EventBus
//!
//! ## 5-Layer Security Architecture
//!
//! ```text
//! Layer 1: Input validation (InjectionDetector patterns)
//! Layer 2: Pipeline analysis (per-segment command blocking)
//! Layer 3: Environment/path isolation (env whitelist, workspace jail)
//! Layer 4: Resource limits (timeout, output cap, session cap, rate limit)
//! Layer 5: Output validation (secret/credential masking)
//! ```

mod config;
mod constants;
mod rate_limit;
mod sanitize;
mod security;
mod session;
mod tool;

#[cfg(test)]
mod tests;

// Re-export public API
pub use config::{BashConfig, BashSecurityMode};
pub use tool::BashTool;
