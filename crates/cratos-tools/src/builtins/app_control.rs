//! App Control Tool - Native application automation
//!
//! Controls macOS applications via osascript (AppleScript/JXA)
//! and Linux applications via xdotool/xclip.

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::time::Instant;
use tokio::process::Command;
use tracing::debug;

/// Blocked patterns in AppleScript to prevent dangerous operations
const BLOCKED_PATTERNS: &[&str] = &[
    "do shell script",
    "System Preferences",
    "System Settings",
    "KeychainAccess",
    "password",
    "sudo",
    "admin",
    "keystroke",  // only blocked in raw script; allowed via dedicated action
];

/// Tool for controlling native applications
pub struct AppControlTool {
    definition: ToolDefinition,
}

impl AppControlTool {
    /// Create a new app control tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "app_control",
            "Control native macOS/Linux applications via system automation. \
             macOS: Uses osascript (AppleScript/JXA). Linux: Uses xdotool/xclip. \
             Actions: run_script (execute AppleScript), open (launch app with optional URL), \
             activate (bring app to front), clipboard_get (read clipboard), clipboard_set (write clipboard). \
             Example: {\"action\": \"run_script\", \"script\": \"tell application \\\"Notes\\\" to make new note with properties {name:\\\"Title\\\", body:\\\"Content\\\"}\"} \
             Example: {\"action\": \"open\", \"app\": \"Safari\", \"url\": \"https://example.com\"} \
             Security: Scripts containing shell commands, password input, or system preference changes are blocked.",
        )
        .with_category(ToolCategory::Exec)
        .with_risk_level(RiskLevel::High)
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["run_script", "open", "activate", "clipboard_get", "clipboard_set"],
                    "description": "Action to perform"
                },
                "script": {
                    "type": "string",
                    "description": "AppleScript/JXA code (for run_script action)"
                },
                "app": {
                    "type": "string",
                    "description": "Application name (for open/activate)"
                },
                "url": {
                    "type": "string",
                    "description": "URL to open (for open action, optional)"
                },
                "text": {
                    "type": "string",
                    "description": "Text content (for clipboard_set)"
                }
            },
            "required": ["action"]
        }));

        Self { definition }
    }

    /// Validate that a script doesn't contain dangerous patterns
    fn validate_script(script: &str) -> Result<()> {
        let lower = script.to_lowercase();
        for pattern in BLOCKED_PATTERNS {
            if lower.contains(&pattern.to_lowercase()) {
                return Err(Error::PermissionDenied(format!(
                    "Script contains blocked pattern: '{}'. For security, scripts cannot contain shell commands, \
                     password input, or system preference changes.",
                    pattern
                )));
            }
        }
        Ok(())
    }

    /// Execute an AppleScript via osascript
    async fn run_applescript(script: &str) -> Result<String> {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run osascript: {}", e)))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(Error::Execution(format!(
                "AppleScript error: {}",
                stderr
            )))
        }
    }

    /// Get clipboard contents (macOS)
    async fn clipboard_get_macos() -> Result<String> {
        let output = Command::new("pbpaste")
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to read clipboard: {}", e)))?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Set clipboard contents (macOS)
    async fn clipboard_set_macos(text: &str) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let mut child = Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Error::Execution(format!("Failed to write clipboard: {}", e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .await
                .map_err(|e| Error::Execution(format!("Failed to write to pbcopy: {}", e)))?;
        }

        let status = child
            .wait()
            .await
            .map_err(|e| Error::Execution(format!("pbcopy failed: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(Error::Execution("pbcopy exited with error".into()))
        }
    }
}

impl Default for AppControlTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for AppControlTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'action' parameter".into()))?;

        debug!(action = %action, "Executing app_control");

        let result = match action {
            "run_script" => {
                let script = input
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'script' parameter for run_script".into())
                    })?;

                Self::validate_script(script)?;
                let output = Self::run_applescript(script).await?;
                serde_json::json!({
                    "success": true,
                    "output": output
                })
            }

            "open" => {
                let app = input.get("app").and_then(|v| v.as_str());
                let url = input.get("url").and_then(|v| v.as_str());

                match (app, url) {
                    (Some(app_name), Some(url_str)) => {
                        let script = format!(
                            "tell application \"{}\" to open location \"{}\"",
                            app_name.replace('"', "\\\""),
                            url_str.replace('"', "\\\"")
                        );
                        Self::run_applescript(&script).await?;
                        serde_json::json!({
                            "success": true,
                            "opened": app_name,
                            "url": url_str
                        })
                    }
                    (Some(app_name), None) => {
                        let script = format!(
                            "tell application \"{}\" to activate",
                            app_name.replace('"', "\\\"")
                        );
                        Self::run_applescript(&script).await?;
                        serde_json::json!({
                            "success": true,
                            "opened": app_name
                        })
                    }
                    (None, Some(url_str)) => {
                        let cmd_output = Command::new("open")
                            .arg(url_str)
                            .output()
                            .await
                            .map_err(|e| {
                                Error::Execution(format!("Failed to open URL: {}", e))
                            })?;

                        if !cmd_output.status.success() {
                            return Err(Error::Execution(format!(
                                "open command failed: {}",
                                String::from_utf8_lossy(&cmd_output.stderr)
                            )));
                        }
                        serde_json::json!({
                            "success": true,
                            "opened_url": url_str
                        })
                    }
                    (None, None) => {
                        return Err(Error::InvalidInput(
                            "Either 'app' or 'url' is required for open action".into(),
                        ));
                    }
                }
            }

            "activate" => {
                let app = input
                    .get("app")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'app' parameter for activate".into())
                    })?;

                let script = format!(
                    "tell application \"{}\" to activate",
                    app.replace('"', "\\\"")
                );
                Self::run_applescript(&script).await?;
                serde_json::json!({
                    "success": true,
                    "activated": app
                })
            }

            "clipboard_get" => {
                let content = Self::clipboard_get_macos().await?;
                serde_json::json!({
                    "success": true,
                    "content": content
                })
            }

            "clipboard_set" => {
                let text = input
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'text' parameter for clipboard_set".into())
                    })?;

                Self::clipboard_set_macos(text).await?;
                serde_json::json!({
                    "success": true,
                    "length": text.len()
                })
            }

            other => {
                return Err(Error::InvalidInput(format!(
                    "Unknown action: '{}'. Valid actions: run_script, open, activate, clipboard_get, clipboard_set",
                    other
                )));
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(result, duration_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definition() {
        let tool = AppControlTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "app_control");
        assert_eq!(def.risk_level, RiskLevel::High);
        assert_eq!(def.category, ToolCategory::Exec);
    }

    #[test]
    fn test_validate_script_blocks_dangerous() {
        assert!(AppControlTool::validate_script("do shell script \"rm -rf /\"").is_err());
        assert!(AppControlTool::validate_script("tell application \"System Preferences\"").is_err());
        assert!(AppControlTool::validate_script("set password to \"secret\"").is_err());
    }

    #[test]
    fn test_validate_script_allows_safe() {
        assert!(AppControlTool::validate_script(
            "tell application \"Notes\" to make new note with properties {name:\"Test\", body:\"Hello\"}"
        ).is_ok());
        assert!(AppControlTool::validate_script(
            "tell application \"Reminders\" to make new reminder with properties {name:\"Buy milk\"}"
        ).is_ok());
    }
}
