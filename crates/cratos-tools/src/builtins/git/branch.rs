//! Git Branch Tool

use super::security::is_valid_branch_name;
use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tracing::debug;

/// Tool for git branch operations
pub struct GitBranchTool {
    definition: ToolDefinition,
}

impl GitBranchTool {
    /// Create a new git branch tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "git_branch",
            "Manage git branches. Actions: list (show all branches with current marked), \
             create (new branch), checkout (switch branch), delete (remove branch). \
             Current branch is marked with * in list output. \
             Example: {\"action\": \"list\"} or {\"action\": \"create\", \"name\": \"feature-x\"} \
             or {\"action\": \"checkout\", \"name\": \"main\"}"
        )
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::Medium)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list", "create", "checkout", "delete"],
                        "description": "Branch action to perform",
                        "default": "list"
                    },
                    "name": {
                        "type": "string",
                        "description": "Branch name (for create/checkout/delete)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    }
                }
            }));

        Self { definition }
    }
}

impl Default for GitBranchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitBranchTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let name = input.get("name").and_then(|v| v.as_str());

        debug!(path = %path, action = %action, "Git branch operation");

        let mut cmd = Command::new("git");
        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        match action {
            "list" => {
                cmd.args([
                    "branch",
                    "-a",
                    "--format=%(refname:short) %(objectname:short) %(upstream:short)",
                ]);
            }
            "create" => {
                let branch_name = name.ok_or_else(|| {
                    Error::InvalidInput("Branch name required for create".to_string())
                })?;
                // SECURITY: Validate branch name
                if !is_valid_branch_name(branch_name) {
                    return Err(Error::InvalidInput(format!(
                        "Invalid branch name: {}",
                        branch_name
                    )));
                }
                cmd.args(["branch", branch_name]);
            }
            "checkout" => {
                let branch_name = name.ok_or_else(|| {
                    Error::InvalidInput("Branch name required for checkout".to_string())
                })?;
                // SECURITY: Validate branch name
                if !is_valid_branch_name(branch_name) {
                    return Err(Error::InvalidInput(format!(
                        "Invalid branch name: {}",
                        branch_name
                    )));
                }
                cmd.args(["checkout", branch_name]);
            }
            "delete" => {
                let branch_name = name.ok_or_else(|| {
                    Error::InvalidInput("Branch name required for delete".to_string())
                })?;
                // SECURITY: Validate branch name
                if !is_valid_branch_name(branch_name) {
                    return Err(Error::InvalidInput(format!(
                        "Invalid branch name: {}",
                        branch_name
                    )));
                }
                // SECURITY: Use -d (safe delete) instead of -D (force delete)
                cmd.args(["branch", "-d", branch_name]);
            }
            _ => {
                return Err(Error::InvalidInput(format!("Unknown action: {}", action)));
            }
        }

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run git branch: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            match action {
                "list" => {
                    let branches: Vec<serde_json::Value> = stdout
                        .lines()
                        .filter(|line| !line.is_empty())
                        .map(|line| {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            serde_json::json!({
                                "name": parts.first().unwrap_or(&""),
                                "commit": parts.get(1).unwrap_or(&""),
                                "upstream": parts.get(2).unwrap_or(&"")
                            })
                        })
                        .collect();

                    Ok(ToolResult::success(
                        serde_json::json!({
                            "action": action,
                            "branches": branches,
                            "count": branches.len()
                        }),
                        duration,
                    ))
                }
                _ => Ok(ToolResult::success(
                    serde_json::json!({
                        "action": action,
                        "name": name,
                        "success": true,
                        "output": stdout
                    }),
                    duration,
                )),
            }
        } else {
            Ok(ToolResult::failure(
                format!("git branch {} failed: {}", action, stderr),
                duration,
            ))
        }
    }
}
