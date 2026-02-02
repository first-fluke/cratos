//! Exec tool - Shell command execution

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tracing::{debug, warn};

/// Tool for executing shell commands (with safety restrictions)
pub struct ExecTool {
    definition: ToolDefinition,
}

impl ExecTool {
    /// Create a new exec tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new("exec", "Execute a shell command")
            .with_category(ToolCategory::Exec)
            .with_risk_level(RiskLevel::High)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to execute"
                    },
                    "args": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Command arguments"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds",
                        "default": 60
                    }
                },
                "required": ["command"]
            }));

        Self { definition }
    }
}

impl Default for ExecTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for ExecTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'command' parameter".to_string()))?;

        let args: Vec<String> = input
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let cwd = input.get("cwd").and_then(|v| v.as_str());

        let timeout_secs = input
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(60);

        debug!(command = %command, args = ?args, "Executing command");

        // Build the command
        let mut cmd = Command::new(command);
        cmd.args(&args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        // Execute with timeout
        let child = cmd.spawn().map_err(|e| {
            warn!(command = %command, error = %e, "Failed to spawn command");
            Error::Execution(e.to_string())
        })?;

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| Error::Timeout(timeout_secs * 1000))?
        .map_err(|e| Error::Execution(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            Ok(ToolResult::success(
                serde_json::json!({
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": exit_code,
                    "command": command
                }),
                duration,
            ))
        } else {
            Ok(ToolResult {
                success: false,
                output: serde_json::json!({
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": exit_code,
                    "command": command
                }),
                error: Some(format!("Command exited with code {}", exit_code)),
                duration_ms: duration,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Tool;

    #[test]
    fn test_exec_definition() {
        let tool = ExecTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "exec");
        assert_eq!(def.risk_level, RiskLevel::High);
        assert_eq!(def.category, ToolCategory::Exec);
    }
}
