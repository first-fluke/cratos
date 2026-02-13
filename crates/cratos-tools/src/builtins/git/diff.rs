//! Git Diff Tool

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tracing::debug;

/// Tool for showing git diffs
pub struct GitDiffTool {
    definition: ToolDefinition,
}

impl GitDiffTool {
    /// Create a new git diff tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new("git_diff", "Show git diff")
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::Low)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "staged": {
                        "type": "boolean",
                        "description": "Show staged changes only",
                        "default": false
                    },
                    "commit": {
                        "type": "string",
                        "description": "Compare with specific commit"
                    },
                    "file": {
                        "type": "string",
                        "description": "Show diff for specific file"
                    },
                    "stat": {
                        "type": "boolean",
                        "description": "Show diffstat only",
                        "default": false
                    }
                }
            }));

        Self { definition }
    }
}

impl Default for GitDiffTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitDiffTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let staged = input
            .get("staged")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let commit = input.get("commit").and_then(|v| v.as_str());
        let file = input.get("file").and_then(|v| v.as_str());

        let stat = input.get("stat").and_then(|v| v.as_bool()).unwrap_or(false);

        debug!(path = %path, "Getting git diff");

        let mut cmd = Command::new("git");
        cmd.arg("diff");

        if staged {
            cmd.arg("--staged");
        }

        if stat {
            cmd.arg("--stat");
        }

        if let Some(c) = commit {
            cmd.arg(c);
        }

        if let Some(f) = file {
            cmd.arg("--");
            cmd.arg(f);
        }

        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run git diff: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            Ok(ToolResult::success(
                serde_json::json!({
                    "diff": stdout,
                    "has_changes": !stdout.is_empty(),
                    "staged": staged
                }),
                duration,
            ))
        } else {
            Ok(ToolResult::failure(
                format!("git diff failed: {}", stderr),
                duration,
            ))
        }
    }
}
