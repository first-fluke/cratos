//! Agent CLI tool — delegate tasks to external AI coding agents
//!
//! Supports Claude Code, Codex, Gemini CLI, and Antigravity as
//! whitelisted agents. Each invocation is a one-shot execution
//! (no interactive session) via `tokio::process::Command`.

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Default execution timeout (seconds)
const DEFAULT_TIMEOUT_SECS: u64 = 300;

/// Maximum output size in bytes (50 KB)
const MAX_OUTPUT_BYTES: usize = 50 * 1024;

/// Configuration for a single external agent.
#[derive(Debug, Clone)]
struct AgentConfig {
    /// Base command (e.g. "claude", "codex")
    command: String,
    /// Argument template — `{prompt}` is replaced with the user prompt.
    /// Other entries are passed as-is.
    args_template: Vec<String>,
    /// Per-agent timeout override
    timeout_secs: u64,
}

/// Built-in tool that delegates coding tasks to external AI CLI agents.
pub struct AgentCliTool {
    definition: ToolDefinition,
    agents: HashMap<String, AgentConfig>,
}

impl AgentCliTool {
    /// Create with the default set of supported agents.
    #[must_use]
    pub fn new() -> Self {
        let mut agents = HashMap::new();

        agents.insert(
            "claude".to_string(),
            AgentConfig {
                command: "claude".to_string(),
                args_template: vec!["--print".to_string(), "{prompt}".to_string()],
                timeout_secs: DEFAULT_TIMEOUT_SECS,
            },
        );

        agents.insert(
            "codex".to_string(),
            AgentConfig {
                command: "codex".to_string(),
                args_template: vec!["exec".to_string(), "{prompt}".to_string()],
                timeout_secs: DEFAULT_TIMEOUT_SECS,
            },
        );

        agents.insert(
            "gemini".to_string(),
            AgentConfig {
                command: "gemini".to_string(),
                args_template: vec!["-p".to_string(), "{prompt}".to_string()],
                timeout_secs: DEFAULT_TIMEOUT_SECS,
            },
        );

        agents.insert(
            "antigravity".to_string(),
            AgentConfig {
                command: "ag".to_string(),
                args_template: vec!["run".to_string(), "{prompt}".to_string()],
                timeout_secs: DEFAULT_TIMEOUT_SECS,
            },
        );

        let definition = ToolDefinition::new(
            "agent_cli",
            "Delegate a coding task to an external AI agent (Claude Code, Codex, Gemini CLI, \
             Antigravity). The agent runs as a one-shot CLI invocation and returns its output.",
        )
        .with_category(ToolCategory::Exec)
        .with_risk_level(RiskLevel::High)
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "enum": ["claude", "codex", "gemini", "antigravity"],
                    "description": "Which AI agent to invoke"
                },
                "prompt": {
                    "type": "string",
                    "description": "Task description / prompt to send to the agent"
                },
                "workspace": {
                    "type": "string",
                    "description": "Optional working directory for the agent"
                }
            },
            "required": ["agent", "prompt"]
        }));

        Self { definition, agents }
    }
}

impl Default for AgentCliTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for AgentCliTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        // --- Parse parameters ---
        let agent_name = input
            .get("agent")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'agent' parameter".to_string()))?;

        let prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'prompt' parameter".to_string()))?;

        if prompt.trim().is_empty() {
            return Err(Error::InvalidInput("Prompt must not be empty".to_string()));
        }

        let workspace = input
            .get("workspace")
            .and_then(|v| v.as_str())
            .map(String::from);

        // --- Whitelist check ---
        let config = self.agents.get(agent_name).ok_or_else(|| {
            Error::InvalidInput(format!(
                "Unknown agent '{}'. Supported: {}",
                agent_name,
                self.agents.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        })?;

        // --- Verify binary exists ---
        let which_output = tokio::process::Command::new("which")
            .arg(&config.command)
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run `which`: {}", e)))?;

        if !which_output.status.success() {
            return Ok(ToolResult::failure(
                format!(
                    "Agent '{}' binary ('{}') not found in PATH. Please install it first.",
                    agent_name, config.command
                ),
                start.elapsed().as_millis() as u64,
            ));
        }

        // --- Build command ---
        let args: Vec<String> = config
            .args_template
            .iter()
            .map(|a| a.replace("{prompt}", prompt))
            .collect();

        info!(
            agent = %agent_name,
            command = %config.command,
            workspace = ?workspace,
            "Invoking external AI agent"
        );
        debug!(args = ?args, "Agent CLI arguments");

        let mut cmd = tokio::process::Command::new(&config.command);
        cmd.args(&args);

        // Set working directory if provided
        if let Some(ref dir) = workspace {
            let path = std::path::Path::new(dir);
            if !path.is_dir() {
                return Err(Error::InvalidInput(format!(
                    "Workspace directory does not exist: {}",
                    dir
                )));
            }
            cmd.current_dir(path);
        }

        // Prevent the child from inheriting stdin (non-interactive)
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // --- Execute with timeout ---
        let timeout = std::time::Duration::from_secs(config.timeout_secs);
        let result = tokio::time::timeout(timeout, cmd.output()).await;

        let output = match result {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Ok(ToolResult::failure(
                    format!("Failed to execute '{}': {}", config.command, e),
                    start.elapsed().as_millis() as u64,
                ));
            }
            Err(_) => {
                return Ok(ToolResult::failure(
                    format!(
                        "Agent '{}' timed out after {}s",
                        agent_name, config.timeout_secs
                    ),
                    start.elapsed().as_millis() as u64,
                ));
            }
        };

        let duration = start.elapsed().as_millis() as u64;
        let exit_code = output.status.code().unwrap_or(-1);

        // --- Process output (UTF-8 safe truncation) ---
        let stdout = truncate_utf8(&output.stdout, MAX_OUTPUT_BYTES);
        let stderr = truncate_utf8(&output.stderr, MAX_OUTPUT_BYTES / 4);

        // Mask potential secrets in output
        let stdout = mask_secrets(&stdout);
        let stderr = mask_secrets(&stderr);

        if !output.status.success() {
            warn!(
                agent = %agent_name,
                exit_code = exit_code,
                stderr_len = stderr.len(),
                "Agent exited with non-zero status"
            );
        }

        info!(
            agent = %agent_name,
            exit_code = exit_code,
            stdout_len = stdout.len(),
            duration_ms = duration,
            "Agent execution completed"
        );

        Ok(ToolResult::success(
            serde_json::json!({
                "agent": agent_name,
                "exit_code": exit_code,
                "stdout": stdout,
                "stderr": stderr,
                "duration_ms": duration,
            }),
            duration,
        ))
    }
}

/// Truncate a byte slice to valid UTF-8 within the given byte limit.
fn truncate_utf8(bytes: &[u8], max_bytes: usize) -> String {
    let s = String::from_utf8_lossy(bytes);
    if s.len() <= max_bytes {
        return s.into_owned();
    }
    // Find the last char boundary at or before max_bytes
    let truncated: String = s
        .char_indices()
        .take_while(|(i, _)| *i < max_bytes)
        .map(|(_, c)| c)
        .collect();
    format!("{}...\n[truncated: {} total bytes]", truncated, bytes.len())
}

/// Basic secret masking — replace patterns that look like API keys or tokens.
fn mask_secrets(text: &str) -> String {
    // Mask strings that look like API keys (sk-..., ghp_..., ghu_..., etc.)
    let re = regex::Regex::new(
        r"(sk-[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{20,}|ghu_[a-zA-Z0-9]{20,}|xoxb-[a-zA-Z0-9\-]{20,})",
    )
    .expect("secret mask regex");
    re.replace_all(text, "[REDACTED]").into_owned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let tool = AgentCliTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "agent_cli");
        assert_eq!(def.risk_level, RiskLevel::High);
        assert_eq!(def.category, ToolCategory::Exec);
    }

    #[test]
    fn test_default_agents() {
        let tool = AgentCliTool::new();
        assert!(tool.agents.contains_key("claude"));
        assert!(tool.agents.contains_key("codex"));
        assert!(tool.agents.contains_key("gemini"));
        assert!(tool.agents.contains_key("antigravity"));
        assert_eq!(tool.agents.len(), 4);
    }

    #[test]
    fn test_truncate_utf8_short() {
        let text = "hello world";
        assert_eq!(truncate_utf8(text.as_bytes(), 100), "hello world");
    }

    #[test]
    fn test_truncate_utf8_long() {
        let text = "a".repeat(200);
        let result = truncate_utf8(text.as_bytes(), 50);
        assert!(result.contains("..."));
        assert!(result.contains("[truncated:"));
    }

    #[test]
    fn test_truncate_utf8_multibyte() {
        // Korean characters (3 bytes each in UTF-8)
        let text = "안녕하세요 세계";
        let result = truncate_utf8(text.as_bytes(), 10);
        // Should not panic on multibyte boundary
        assert!(result.contains("...") || result.len() <= 30);
    }

    #[test]
    fn test_mask_secrets() {
        let text = "key is sk-abc123def456ghi789jkl012mno";
        let masked = mask_secrets(text);
        assert!(masked.contains("[REDACTED]"));
        assert!(!masked.contains("sk-abc123"));
    }

    #[test]
    fn test_mask_secrets_no_match() {
        let text = "no secrets here";
        assert_eq!(mask_secrets(text), "no secrets here");
    }

    #[tokio::test]
    async fn test_missing_agent() {
        let tool = AgentCliTool::new();
        let result = tool.execute(serde_json::json!({"prompt": "test"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_prompt() {
        let tool = AgentCliTool::new();
        let result = tool.execute(serde_json::json!({"agent": "claude"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_empty_prompt() {
        let tool = AgentCliTool::new();
        let result = tool
            .execute(serde_json::json!({"agent": "claude", "prompt": "  "}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unknown_agent() {
        let tool = AgentCliTool::new();
        let result = tool
            .execute(serde_json::json!({"agent": "unknown", "prompt": "test"}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_workspace() {
        let tool = AgentCliTool::new();
        let result = tool
            .execute(serde_json::json!({
                "agent": "claude",
                "prompt": "test",
                "workspace": "/nonexistent/path/xyz"
            }))
            .await;
        assert!(result.is_err());
    }
}
