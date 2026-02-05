//! Builtins - Built-in tools for Cratos
//!
//! This module provides the core set of built-in tools:
//! - File tools: file_read, file_write, file_list
//! - HTTP tools: http_get, http_post
//! - Exec tool: exec (shell command execution)
//! - Git tools: git_status, git_commit, git_branch, git_diff
//! - GitHub tool: github_api
//! - Wake-on-LAN tool: wol
//! - Config tool: config (natural language configuration)

mod config;
pub mod config_manager;
mod exec;
mod file;
mod git;
mod github;
mod http;
mod wol;

pub use config::{ConfigAction, ConfigInput, ConfigTarget, ConfigTool};
pub use exec::ExecTool;
pub use file::{FileListTool, FileReadTool, FileWriteTool};
pub use git::{GitBranchTool, GitCommitTool, GitDiffTool, GitPushTool, GitStatusTool};
pub use github::GitHubApiTool;
pub use http::{HttpGetTool, HttpPostTool};
pub use wol::WolTool;

use crate::browser::BrowserTool;
use crate::registry::ToolRegistry;
use std::collections::HashMap;
use std::sync::Arc;

/// Configuration for built-in tools
#[derive(Debug, Clone, Default)]
pub struct BuiltinsConfig {
    /// Named WoL devices (name -> MAC address)
    pub wol_devices: HashMap<String, String>,
}

/// Register all built-in tools with the registry (default config)
pub fn register_builtins(registry: &mut ToolRegistry) {
    register_builtins_with_config(registry, &BuiltinsConfig::default());
}

/// Register all built-in tools with custom configuration
pub fn register_builtins_with_config(registry: &mut ToolRegistry, config: &BuiltinsConfig) {
    // File tools
    registry.register(Arc::new(FileReadTool::new()));
    registry.register(Arc::new(FileWriteTool::new()));
    registry.register(Arc::new(FileListTool::new()));

    // HTTP tools
    registry.register(Arc::new(HttpGetTool::new()));
    registry.register(Arc::new(HttpPostTool::new()));

    // Exec tool
    registry.register(Arc::new(ExecTool::new()));

    // Git tools
    registry.register(Arc::new(GitStatusTool::new()));
    registry.register(Arc::new(GitCommitTool::new()));
    registry.register(Arc::new(GitBranchTool::new()));
    registry.register(Arc::new(GitDiffTool::new()));
    registry.register(Arc::new(GitPushTool::new()));

    // GitHub API tool
    registry.register(Arc::new(GitHubApiTool::new()));

    // Browser automation tool
    registry.register(Arc::new(BrowserTool::new()));

    // Wake-on-LAN tool (with named devices from config)
    registry.register(Arc::new(WolTool::with_devices(config.wol_devices.clone())));

    // Config tool (natural language configuration)
    registry.register(Arc::new(ConfigTool::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_builtins() {
        let mut registry = ToolRegistry::new();
        register_builtins(&mut registry);

        assert!(registry.has("file_read"));
        assert!(registry.has("file_write"));
        assert!(registry.has("file_list"));
        assert!(registry.has("http_get"));
        assert!(registry.has("http_post"));
        assert!(registry.has("exec"));
        assert!(registry.has("git_status"));
        assert!(registry.has("git_commit"));
        assert!(registry.has("git_branch"));
        assert!(registry.has("git_diff"));
        assert!(registry.has("github_api"));
        assert!(registry.has("git_push"));
        assert!(registry.has("browser"));
        assert!(registry.has("wol"));
        assert!(registry.has("config"));
        assert_eq!(registry.len(), 15);
    }
}
