//! Git Clone Tool

use super::security::{is_valid_branch_name, is_valid_clone_path, is_valid_clone_url};
use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tracing::debug;

/// Tool for cloning git repositories
pub struct GitCloneTool {
    definition: ToolDefinition,
}

impl GitCloneTool {
    /// Create a new git clone tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "git_clone",
            "Clone a git repository from a remote URL. Supports https://, git://, and ssh:// protocols. \
             Set depth for shallow clone (e.g. depth=1 for latest snapshot only). \
             Specify branch to clone a specific branch. Target path defaults to repo name in current directory. \
             Example: {\"url\": \"https://github.com/user/repo.git\"} or {\"url\": \"...\", \"depth\": 1, \"branch\": \"dev\"}"
        )
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::Medium)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Repository URL (https://, git://, or ssh:// protocol)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Local path to clone into"
                    },
                    "branch": {
                        "type": "string",
                        "description": "Branch to checkout after cloning"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Shallow clone depth (0 = full clone)",
                        "default": 0
                    }
                },
                "required": ["url"]
            }));

        Self { definition }
    }
}

impl Default for GitCloneTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitCloneTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'url' parameter".to_string()))?;

        // SECURITY: Validate URL
        if !is_valid_clone_url(url) {
            return Err(Error::InvalidInput(
                "Invalid or unsafe repository URL: only https://, git://, ssh:// protocols allowed"
                    .to_string(),
            ));
        }

        let path = input.get("path").and_then(|v| v.as_str());
        let branch = input.get("branch").and_then(|v| v.as_str());
        let depth = input.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);

        // SECURITY: Validate path if provided
        if let Some(p) = path {
            if !is_valid_clone_path(p) {
                return Err(Error::InvalidInput(
                    "Invalid clone path: path traversal or special characters not allowed"
                        .to_string(),
                ));
            }
        }

        // SECURITY: Validate branch name if provided
        if let Some(b) = branch {
            if !is_valid_branch_name(b) {
                return Err(Error::InvalidInput(format!("Invalid branch name: {}", b)));
            }
        }

        debug!(url = %url, path = ?path, branch = ?branch, depth = %depth, "Cloning git repository");

        let mut cmd = Command::new("git");
        cmd.arg("clone");

        if depth > 0 {
            cmd.arg("--depth");
            cmd.arg(depth.to_string());
        }

        if let Some(b) = branch {
            cmd.arg("--branch");
            cmd.arg(b);
        }

        cmd.arg(url);

        if let Some(p) = path {
            cmd.arg(p);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run git clone: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            Ok(ToolResult::success(
                serde_json::json!({
                    "success": true,
                    "url": url,
                    "path": path,
                    "branch": branch,
                    "output": format!("{}{}", stdout, stderr)
                }),
                duration,
            ))
        } else {
            // SECURITY: Sanitize error (don't leak URLs with tokens)
            let sanitized = if stderr.contains("http") || stderr.contains("@") {
                "git clone failed: authentication or remote error".to_string()
            } else {
                format!("git clone failed: {}", stderr)
            };
            Ok(ToolResult::failure(sanitized, duration))
        }
    }
}
