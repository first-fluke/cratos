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

mod agent_cli;
mod bash;
mod config;
pub mod config_manager;
mod exec;
mod file;
mod git;
mod github;
mod http;
mod send_file;
mod web_search;
mod wol;
mod a2ui; // Added

pub use agent_cli::AgentCliTool;
pub use bash::{BashConfig, BashSecurityMode, BashTool};
pub use config::{ConfigAction, ConfigInput, ConfigTarget, ConfigTool};
pub use exec::{ExecConfig, ExecMode, ExecTool};
pub use file::{is_sensitive_file, validate_path, FileListTool, FileReadTool, FileWriteTool};
pub use git::{
    GitBranchTool, GitCloneTool, GitCommitTool, GitDiffTool, GitLogTool, GitPushTool, GitStatusTool,
};
pub use github::GitHubApiTool;
pub use http::{HttpGetTool, HttpPostTool};
pub use send_file::SendFileTool;
pub use web_search::WebSearchTool;
pub use wol::WolTool;
pub use a2ui::{A2uiRenderTool, A2uiWaitEventTool}; // Added

use crate::browser::BrowserTool;
use crate::registry::ToolRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use cratos_canvas::a2ui::{A2uiSessionManager, A2uiSecurityPolicy}; // Added

/// Configuration for built-in tools
#[derive(Debug, Clone, Default)]
pub struct BuiltinsConfig {
    /// Named WoL devices (name -> MAC address)
    pub wol_devices: HashMap<String, String>,
    /// Exec tool security configuration
    pub exec: ExecConfig,
    /// Bash tool (PTY) security configuration
    pub bash: BashConfig,
    /// A2UI Session Manager (Optional, enables A2UI tools)
    pub a2ui_manager: Option<Arc<A2uiSessionManager>>, // Added
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

    // Exec tool (with configurable security)
    registry.register(Arc::new(ExecTool::with_config(config.exec.clone())));

    // Git tools
    registry.register(Arc::new(GitStatusTool::new()));
    registry.register(Arc::new(GitCommitTool::new()));
    registry.register(Arc::new(GitBranchTool::new()));
    registry.register(Arc::new(GitDiffTool::new()));
    registry.register(Arc::new(GitPushTool::new()));

    // Git clone and log tools
    registry.register(Arc::new(GitCloneTool::new()));
    registry.register(Arc::new(GitLogTool::new()));

    // GitHub API tool
    registry.register(Arc::new(GitHubApiTool::new()));

    // Browser automation tool
    registry.register(Arc::new(BrowserTool::new()));

    // Wake-on-LAN tool (with named devices from config)
    registry.register(Arc::new(WolTool::with_devices(config.wol_devices.clone())));

    // Config tool (natural language configuration)
    registry.register(Arc::new(ConfigTool::new()));

    // Web search tool (DuckDuckGo, no API key required)
    registry.register(Arc::new(WebSearchTool::new()));

    // Agent CLI tool (delegate tasks to external AI agents)
    registry.register(Arc::new(AgentCliTool::new()));

    // Bash tool (PTY-based, full shell support)
    registry.register(Arc::new(BashTool::with_config(config.bash.clone())));

    // Send file tool (prepares files as artifacts for channel delivery)
    registry.register(Arc::new(SendFileTool::new()));

    // A2UI Tools (Only if manager is provided) // Added
    if let Some(manager) = &config.a2ui_manager {
        // Use default security policy if not provided (could add to config later)
        let policy = Arc::new(A2uiSecurityPolicy::default_restrictive());
        
        registry.register(Arc::new(A2uiRenderTool::new(
            manager.clone(),
            policy,
        )));
        
        registry.register(Arc::new(A2uiWaitEventTool::new(manager.clone())));
    }
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
        assert!(registry.has("web_search"));
        assert!(registry.has("bash"));
        assert!(registry.has("git_clone"));
        assert!(registry.has("git_log"));
        assert!(registry.has("agent_cli"));
        assert!(registry.has("send_file"));
        // A2UI tools are NOT registered by default, so count remains 21
        assert_eq!(registry.len(), 21);
    }
}
