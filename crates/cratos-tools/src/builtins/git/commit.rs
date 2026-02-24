//! Git Commit Tool

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tracing::debug;

/// Tool for creating git commits
pub struct GitCommitTool {
    definition: ToolDefinition,
}

impl GitCommitTool {
    /// Create a new git commit tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "git_commit",
            "Create a git commit. Requires a commit message. Set add_all=true to stage all changes before committing, \
             or specify files to stage specific paths. Always check git_status first to confirm staged changes. \
             Example: {\"message\": \"feat: add login\", \"add_all\": true} or {\"message\": \"fix typo\", \"files\": [\"README.md\"]}"
        )
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::Medium)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Commit message"
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "add_all": {
                        "type": "boolean",
                        "description": "Stage all changes before committing",
                        "default": false
                    },
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Specific files to stage before committing"
                    }
                },
                "required": ["message"]
            }));

        Self { definition }
    }
}

impl Default for GitCommitTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitCommitTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let message = input
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'message' parameter".to_string()))?;

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let add_all = input
            .get("add_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let files: Vec<String> = input
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        debug!(path = %path, message = %message, "Creating git commit");

        // Stage files if requested
        if add_all {
            let mut add_cmd = Command::new("git");
            add_cmd.args(["add", "-A"]);
            add_cmd.current_dir(path);
            add_cmd
                .output()
                .await
                .map_err(|e| Error::Execution(format!("Failed to stage all files: {}", e)))?;
        } else if !files.is_empty() {
            let mut add_cmd = Command::new("git");
            add_cmd.arg("add");
            add_cmd.args(&files);
            add_cmd.current_dir(path);
            add_cmd
                .output()
                .await
                .map_err(|e| Error::Execution(format!("Failed to stage files: {}", e)))?;
        }

        // Create commit
        let mut cmd = Command::new("git");
        cmd.args(["commit", "-m", message]);
        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to create commit: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            // Extract commit hash from output
            let commit_hash = stdout
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .map(|s| s.trim_matches(|c| c == '[' || c == ']'))
                .unwrap_or("")
                .to_string();

            Ok(ToolResult::success(
                serde_json::json!({
                    "success": true,
                    "commit": commit_hash,
                    "message": message,
                    "output": stdout
                }),
                duration,
            ))
        } else {
            Ok(ToolResult::failure(
                format!("git commit failed: {}", stderr),
                duration,
            ))
        }
    }
}
