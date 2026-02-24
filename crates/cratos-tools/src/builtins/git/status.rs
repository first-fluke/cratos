//! Git Status Tool

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tracing::debug;

/// Tool for getting git repository status
pub struct GitStatusTool {
    definition: ToolDefinition,
}

impl GitStatusTool {
    /// Create a new git status tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "git_status",
            "Get the status of a git repository. Shows staged, unstaged, and untracked files. \
             Set short=true for compact output. Returns current branch name and tracking info. \
             Use before committing to verify what will be included. \
             Example: {\"path\": \".\"} or {\"path\": \"/project\", \"short\": true}"
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
                    "short": {
                        "type": "boolean",
                        "description": "Use short format output",
                        "default": false
                    }
                }
            }));

        Self { definition }
    }
}

impl Default for GitStatusTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitStatusTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let short = input
            .get("short")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        debug!(path = %path, "Getting git status");

        let mut cmd = Command::new("git");
        cmd.arg("status");
        if short {
            cmd.arg("--short");
        }
        cmd.arg("--porcelain=v1");
        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run git status: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            // Parse porcelain output
            let files: Vec<serde_json::Value> = stdout
                .lines()
                .filter(|line| !line.is_empty())
                .map(|line| {
                    let status = &line[0..2];
                    let file = line[3..].to_string();
                    serde_json::json!({
                        "status": status.trim(),
                        "file": file
                    })
                })
                .collect();

            Ok(ToolResult::success(
                serde_json::json!({
                    "clean": files.is_empty(),
                    "files": files,
                    "count": files.len()
                }),
                duration,
            ))
        } else {
            Ok(ToolResult::failure(
                format!("git status failed: {}", stderr),
                duration,
            ))
        }
    }
}
