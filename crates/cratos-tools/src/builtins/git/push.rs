//! Git Push Tool

use super::security::is_valid_branch_name;
use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tracing::debug;

/// Tool for pushing to remote repositories (with security restrictions)
pub struct GitPushTool {
    definition: ToolDefinition,
}

impl GitPushTool {
    /// Create a new git push tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "git_push",
            "Push commits to a remote git repository. Defaults to 'origin' remote and current branch. \
             Set set_upstream=true for new branches. Force push is not supported for safety. \
             Specify remote and branch to push to a specific target. \
             Example: {\"path\": \".\"} or {\"remote\": \"origin\", \"branch\": \"feature-x\", \"set_upstream\": true}"
        )
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::High)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "remote": {
                        "type": "string",
                        "description": "Remote name (default: origin)",
                        "default": "origin"
                    },
                    "branch": {
                        "type": "string",
                        "description": "Branch to push (default: current branch)"
                    },
                    "set_upstream": {
                        "type": "boolean",
                        "description": "Set upstream tracking",
                        "default": false
                    }
                }
            }));

        Self { definition }
    }
}

impl Default for GitPushTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitPushTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let remote = input
            .get("remote")
            .and_then(|v| v.as_str())
            .unwrap_or("origin");
        let branch = input.get("branch").and_then(|v| v.as_str());
        let set_upstream = input
            .get("set_upstream")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // SECURITY: Validate remote name
        if !is_valid_branch_name(remote) {
            return Err(Error::InvalidInput(format!(
                "Invalid remote name: {}",
                remote
            )));
        }

        // SECURITY: Validate branch name if provided
        if let Some(b) = branch {
            if !is_valid_branch_name(b) {
                return Err(Error::InvalidInput(format!("Invalid branch name: {}", b)));
            }
        }

        debug!(path = %path, remote = %remote, branch = ?branch, "Git push");

        let mut cmd = Command::new("git");
        cmd.arg("push");

        // SECURITY: Never allow force push flags
        // The tool intentionally does not expose --force, -f, or --force-with-lease

        if set_upstream {
            cmd.arg("-u");
        }

        cmd.arg(remote);

        if let Some(b) = branch {
            cmd.arg(b);
        }

        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run git push: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            Ok(ToolResult::success(
                serde_json::json!({
                    "success": true,
                    "remote": remote,
                    "branch": branch,
                    "output": format!("{}{}", stdout, stderr)
                }),
                duration,
            ))
        } else {
            // SECURITY: Sanitize error message (don't leak remote URLs with tokens)
            let sanitized_error = if stderr.contains("http") || stderr.contains("@") {
                "git push failed: authentication or remote error".to_string()
            } else {
                format!("git push failed: {}", stderr)
            };

            Ok(ToolResult::failure(sanitized_error, duration))
        }
    }
}
