//! Builtins - Built-in tools for Cratos
//!
//! This module provides the core set of built-in tools:
//! - File tools: file_read, file_write, file_list
//! - HTTP tools: http_get, http_post
//! - Exec tool: exec (shell command execution)
//! - Git tools: git_status, git_commit, git_branch, git_diff
//! - GitHub tool: github_api

mod exec;
mod file;
mod git;
mod github;
mod http;

pub use exec::ExecTool;
pub use file::{FileListTool, FileReadTool, FileWriteTool};
pub use git::{GitBranchTool, GitCommitTool, GitDiffTool, GitPushTool, GitStatusTool};
pub use github::GitHubApiTool;
pub use http::{HttpGetTool, HttpPostTool};

use crate::browser::BrowserTool;
use crate::registry::ToolRegistry;
use std::sync::Arc;

/// Register all built-in tools with the registry
pub fn register_builtins(registry: &mut ToolRegistry) {
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
        assert_eq!(registry.len(), 13);
    }
}
