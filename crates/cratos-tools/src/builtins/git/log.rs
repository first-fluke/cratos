//! Git Log Tool

use super::security::is_valid_branch_name;
use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tracing::debug;

/// Tool for viewing git commit history
pub struct GitLogTool {
    definition: ToolDefinition,
}

impl GitLogTool {
    /// Create a new git log tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "git_log",
            "View git commit history. Returns commit hash, author, date, and message. \
             Set limit to control number of entries (default: 10). \
             Use format: oneline/short/full for different detail levels. Specify branch to view a specific branch's history. \
             Example: {\"limit\": 5} or {\"branch\": \"main\", \"format\": \"oneline\", \"limit\": 20}"
        )
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::Low)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of commits to show",
                        "default": 10
                    },
                    "format": {
                        "type": "string",
                        "enum": ["oneline", "medium"],
                        "description": "Output format",
                        "default": "oneline"
                    },
                    "branch": {
                        "type": "string",
                        "description": "Branch to show history for"
                    }
                }
            }));

        Self { definition }
    }
}

impl Default for GitLogTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitLogTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(100); // Cap at 100

        let format = input
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("oneline");

        let branch = input.get("branch").and_then(|v| v.as_str());

        // SECURITY: Validate branch name if provided
        if let Some(b) = branch {
            if !is_valid_branch_name(b) {
                return Err(Error::InvalidInput(format!("Invalid branch name: {}", b)));
            }
        }

        debug!(path = %path, limit = %limit, format = %format, "Getting git log");

        let mut cmd = Command::new("git");
        cmd.arg("log");

        let fmt_arg = match format {
            "medium" => "--format=medium",
            _ => "--format=%h %s (%an, %ar)",
        };
        cmd.arg(fmt_arg);

        cmd.arg(format!("-n{}", limit));

        if let Some(b) = branch {
            cmd.arg(b);
        }

        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run git log: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            let commits: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
            Ok(ToolResult::success(
                serde_json::json!({
                    "commits": commits,
                    "count": commits.len(),
                    "branch": branch,
                    "format": format
                }),
                duration,
            ))
        } else {
            Ok(ToolResult::failure(
                format!("git log failed: {}", stderr),
                duration,
            ))
        }
    }
}
