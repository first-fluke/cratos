use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::time::Instant;
use tracing::{debug, warn};

use super::config::{ExecConfig, ExecHost};
use super::runner;
use super::security;

/// Tool for executing shell commands (with safety restrictions)
pub struct ExecTool {
    pub(crate) definition: ToolDefinition,
    pub(crate) config: ExecConfig,
}

impl ExecTool {
    /// Create a new exec tool with default config
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ExecConfig::default())
    }

    /// Create a new exec tool with custom config
    #[must_use]
    pub fn with_config(config: ExecConfig) -> Self {
        let timeout_desc = format!("Timeout in seconds (max {})", config.max_timeout_secs);
        let definition = ToolDefinition::new("exec", "Execute a command directly on the user's local machine. No shell pipes or chaining — use separate calls for each command. Example: command=\"ps\" args=[\"aux\"] to list processes.")
            .with_category(ToolCategory::Exec)
            .with_risk_level(RiskLevel::High)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The executable name or path (e.g. \"ps\", \"ls\", \"cat\", \"grep\"). Can include arguments like \"ps aux\" which will be auto-split."
                    },
                    "args": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Command arguments as separate strings, e.g. [\"-la\", \"/tmp\"]. Pipes and shell operators are NOT supported."
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": timeout_desc
                    },
                    "host": {
                        "type": "string",
                        "enum": ["local", "sandbox"],
                        "description": "Execution target: 'local' (default) runs on the host, 'sandbox' runs in an isolated Docker container"
                    }
                },
                "required": ["command"]
            }));

        Self { definition, config }
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

        let raw_command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'command' parameter".to_string()))?;

        // Split command on whitespace: "ps aux" → program="ps", prefix_args=["aux"]
        let mut parts = raw_command.split_whitespace();
        let command = parts.next().unwrap_or(raw_command);
        let prefix_args: Vec<&str> = parts.collect();

        // SECURITY: Check if command is blocked
        if security::is_command_blocked(&self.config, command) {
            warn!(command = %command, "Blocked dangerous command attempt");
            return Err(Error::PermissionDenied(format!(
                "Command '{}' is blocked for security reasons",
                command.split('/').next_back().unwrap_or(command)
            )));
        }

        // SECURITY: Check for shell metacharacters in command (early check)
        if let Some(c) = security::contains_shell_metacharacters(command) {
            warn!(
                command = %command,
                metachar = %c,
                "Command injection attempt blocked"
            );
            return Err(Error::PermissionDenied(format!(
                "Command contains blocked shell metacharacter '{}'. \
                 Direct shell commands are not allowed.",
                c
            )));
        }

        // Check prefix args from command string for metacharacters too
        for arg in &prefix_args {
            if let Some(c) = security::contains_shell_metacharacters(arg) {
                warn!(arg = %arg, metachar = %c, "Command injection in split args");
                return Err(Error::PermissionDenied(format!(
                    "Command contains blocked shell metacharacter '{}'.",
                    c
                )));
            }
        }

        let user_args: Vec<String> = input
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Combine prefix args (from command split) + user-provided args
        let args: Vec<String> = prefix_args
            .iter()
            .map(|s| s.to_string())
            .chain(user_args)
            .collect();

        // SECURITY: Check args for dangerous patterns
        for arg in &args {
            if (arg.contains("..") || arg.starts_with('/'))
                && security::is_path_dangerous(&self.config, arg)
            {
                warn!(arg = %arg, "Blocked dangerous path in argument");
                return Err(Error::PermissionDenied(format!(
                    "Argument '{}' references a restricted path",
                    arg
                )));
            }
        }

        let cwd = input.get("cwd").and_then(|v| v.as_str());

        // SECURITY: Validate working directory
        if let Some(dir) = cwd {
            if security::is_path_dangerous(&self.config, dir) {
                warn!(cwd = %dir, "Blocked dangerous working directory");
                return Err(Error::PermissionDenied(format!(
                    "Working directory '{}' is restricted",
                    dir
                )));
            }
        }

        // SECURITY: Cap timeout to prevent resource exhaustion
        let max_timeout = self.config.max_timeout_secs;
        let timeout_secs = input
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(max_timeout)
            .min(max_timeout);

        // Parse execution host
        let host = match input.get("host").and_then(|v| v.as_str()) {
            Some("sandbox") => ExecHost::Sandbox,
            _ => ExecHost::Local,
        };

        debug!(command = %command, args = ?args, host = ?host, "Executing command");

        let (stdout, stderr, exit_code, success) =
            runner::run_command(&self.config, host, command, &args, cwd, timeout_secs).await?;

        let duration = start.elapsed().as_millis() as u64;

        if success {
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
