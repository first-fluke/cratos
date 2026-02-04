//! Browser tool implementation
//!
//! Provides a unified browser automation tool that wraps MCP browser servers.

use crate::error::{Error, Result};
use crate::mcp::{McpClient, McpServerConfig, McpTransport};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::actions::{BrowserAction, BrowserActionResult};
use super::config::BrowserConfig;

/// Browser automation tool
///
/// This tool provides browser automation capabilities by connecting to
/// MCP browser servers (Playwright or Puppeteer).
pub struct BrowserTool {
    definition: ToolDefinition,
    config: BrowserConfig,
    mcp_client: Arc<RwLock<Option<McpClient>>>,
}

impl BrowserTool {
    /// Create a new browser tool with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(BrowserConfig::default())
    }

    /// Create a new browser tool with custom configuration
    #[must_use]
    pub fn with_config(config: BrowserConfig) -> Self {
        let definition = ToolDefinition::new(
            "browser",
            "Automate browser interactions: navigate, click, type, screenshot, and more",
        )
        .with_category(ToolCategory::External)
        .with_risk_level(RiskLevel::Medium)
        .with_capability("browser")
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": [
                        "navigate", "click", "type", "fill", "screenshot",
                        "get_text", "get_html", "get_attribute",
                        "wait_for_selector", "wait_for_navigation",
                        "evaluate", "select", "check", "hover", "press", "scroll",
                        "get_url", "get_title", "go_back", "go_forward", "reload", "close"
                    ]
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (for navigate action)"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for element actions"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type (for type action)"
                },
                "value": {
                    "type": "string",
                    "description": "Value to fill or select"
                },
                "script": {
                    "type": "string",
                    "description": "JavaScript code to evaluate"
                },
                "key": {
                    "type": "string",
                    "description": "Keyboard key to press"
                },
                "attribute": {
                    "type": "string",
                    "description": "Attribute name to get"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in milliseconds"
                },
                "full_page": {
                    "type": "boolean",
                    "description": "Capture full page screenshot"
                },
                "x": {
                    "type": "integer",
                    "description": "Horizontal scroll amount"
                },
                "y": {
                    "type": "integer",
                    "description": "Vertical scroll amount"
                }
            },
            "required": ["action"]
        }));

        Self {
            definition,
            config,
            mcp_client: Arc::new(RwLock::new(None)),
        }
    }

    /// Ensure MCP client is connected
    async fn ensure_connected(&self) -> Result<()> {
        let mut client_guard = self.mcp_client.write().await;

        if client_guard.is_some() {
            return Ok(());
        }

        info!("Starting browser MCP server");

        let engine = &self.config.default_engine;
        let (command, args) = engine.mcp_command();

        let server_config = McpServerConfig {
            name: engine.server_name().to_string(),
            transport: McpTransport::Stdio {
                command: command.to_string(),
                args: args.iter().map(|s| (*s).to_string()).collect(),
                env: Default::default(),
            },
            auto_start: true,
        };

        let mut client = McpClient::new();
        client.add_server(server_config).await.map_err(|e| {
            warn!(error = %e, "Failed to start browser MCP server");
            Error::Execution(format!("Failed to start browser MCP server: {}", e))
        })?;

        *client_guard = Some(client);
        info!(engine = ?engine, "Browser MCP server started");

        Ok(())
    }

    /// Execute a browser action
    async fn execute_action(&self, action: BrowserAction) -> Result<BrowserActionResult> {
        self.ensure_connected().await?;

        let client_guard = self.mcp_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| Error::Execution("Browser MCP client not initialized".to_string()))?;

        let server_name = self.config.default_engine.server_name();
        let tool_name = action.mcp_tool_name();
        let args = action.to_mcp_args();

        debug!(
            server = %server_name,
            tool = %tool_name,
            args = %args,
            "Executing browser action"
        );

        let result = client
            .call_tool(server_name, tool_name, args)
            .await
            .map_err(|e| Error::Execution(format!("Browser action failed: {}", e)))?;

        if result.is_error {
            let error_text = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(BrowserActionResult::failure(error_text));
        }

        // Extract result data
        let data: serde_json::Value = result
            .content
            .iter()
            .filter_map(|c| c.as_text())
            .next()
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_else(|| {
                serde_json::json!({
                    "text": result.content.iter()
                        .filter_map(|c| c.as_text())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
            });

        // Check for screenshot data
        let screenshot = result.content.iter().find_map(|c| {
            if let crate::mcp::McpContent::Image { data, .. } = c {
                Some(data.clone())
            } else {
                None
            }
        });

        let mut action_result = BrowserActionResult::success(data);
        if let Some(ss) = screenshot {
            action_result = action_result.with_screenshot(ss);
        }

        Ok(action_result)
    }

    /// Parse input JSON into a BrowserAction
    fn parse_action(&self, input: &serde_json::Value) -> Result<BrowserAction> {
        let action_str = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'action' parameter".to_string()))?;

        match action_str {
            "navigate" => {
                let url = input
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'url' for navigate".to_string()))?;
                Ok(BrowserAction::Navigate {
                    url: url.to_string(),
                    wait_until_loaded: input
                        .get("wait_until_loaded")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                })
            }
            "click" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for click".to_string())
                    })?;
                Ok(BrowserAction::Click {
                    selector: selector.to_string(),
                    button: input
                        .get("button")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                })
            }
            "type" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for type".to_string())
                    })?;
                let text = input
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'text' for type".to_string()))?;
                Ok(BrowserAction::Type {
                    selector: selector.to_string(),
                    text: text.to_string(),
                    delay: input.get("delay").and_then(|v| v.as_u64()),
                })
            }
            "fill" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for fill".to_string())
                    })?;
                let value = input
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'value' for fill".to_string()))?;
                Ok(BrowserAction::Fill {
                    selector: selector.to_string(),
                    value: value.to_string(),
                })
            }
            "screenshot" => Ok(BrowserAction::Screenshot {
                path: input.get("path").and_then(|v| v.as_str()).map(String::from),
                full_page: input
                    .get("full_page")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                selector: input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            }),
            "get_text" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for get_text".to_string())
                    })?;
                Ok(BrowserAction::GetText {
                    selector: selector.to_string(),
                })
            }
            "get_html" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for get_html".to_string())
                    })?;
                Ok(BrowserAction::GetHtml {
                    selector: selector.to_string(),
                    outer: input.get("outer").and_then(|v| v.as_bool()).unwrap_or(true),
                })
            }
            "get_attribute" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for get_attribute".to_string())
                    })?;
                let attribute =
                    input
                        .get("attribute")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            Error::InvalidInput("Missing 'attribute' for get_attribute".to_string())
                        })?;
                Ok(BrowserAction::GetAttribute {
                    selector: selector.to_string(),
                    attribute: attribute.to_string(),
                })
            }
            "wait_for_selector" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for wait_for_selector".to_string())
                    })?;
                Ok(BrowserAction::WaitForSelector {
                    selector: selector.to_string(),
                    timeout: input
                        .get("timeout")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(super::DEFAULT_BROWSER_TIMEOUT_MS),
                    visible: input
                        .get("visible")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                })
            }
            "wait_for_navigation" => Ok(BrowserAction::WaitForNavigation {
                timeout: input
                    .get("timeout")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(super::DEFAULT_BROWSER_TIMEOUT_MS),
            }),
            "evaluate" => {
                let script = input
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'script' for evaluate".to_string())
                    })?;
                Ok(BrowserAction::Evaluate {
                    script: script.to_string(),
                })
            }
            "select" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for select".to_string())
                    })?;
                let value = input
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'value' for select".to_string()))?;
                Ok(BrowserAction::Select {
                    selector: selector.to_string(),
                    value: value.to_string(),
                })
            }
            "check" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for check".to_string())
                    })?;
                Ok(BrowserAction::Check {
                    selector: selector.to_string(),
                    checked: input
                        .get("checked")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                })
            }
            "hover" => {
                let selector = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'selector' for hover".to_string())
                    })?;
                Ok(BrowserAction::Hover {
                    selector: selector.to_string(),
                })
            }
            "press" => {
                let key = input
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'key' for press".to_string()))?;
                Ok(BrowserAction::Press {
                    key: key.to_string(),
                    count: input.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as u32,
                })
            }
            "scroll" => Ok(BrowserAction::Scroll {
                selector: input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                x: input.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                y: input.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            }),
            "get_url" => Ok(BrowserAction::GetUrl),
            "get_title" => Ok(BrowserAction::GetTitle),
            "go_back" => Ok(BrowserAction::GoBack),
            "go_forward" => Ok(BrowserAction::GoForward),
            "reload" => Ok(BrowserAction::Reload),
            "close" => Ok(BrowserAction::Close),
            _ => Err(Error::InvalidInput(format!(
                "Unknown action: {}",
                action_str
            ))),
        }
    }
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for BrowserTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        // Check if browser tool is enabled
        if !self.config.enabled {
            return Ok(ToolResult::failure(
                "Browser tool is disabled in configuration",
                start.elapsed().as_millis() as u64,
            ));
        }

        // Parse the action
        let action = self.parse_action(&input)?;
        debug!(action = ?action.name(), "Executing browser action");

        // Execute the action
        let result = self.execute_action(action).await?;
        let duration = start.elapsed().as_millis() as u64;

        if result.success {
            Ok(ToolResult::success(
                serde_json::json!({
                    "data": result.data,
                    "screenshot": result.screenshot
                }),
                duration,
            ))
        } else {
            Ok(ToolResult {
                success: false,
                output: serde_json::json!({
                    "error": result.error
                }),
                error: result.error,
                duration_ms: duration,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_tool_definition() {
        let tool = BrowserTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "browser");
        assert_eq!(def.risk_level, RiskLevel::Medium);
        assert_eq!(def.category, ToolCategory::External);
    }

    #[test]
    fn test_parse_navigate_action() {
        let tool = BrowserTool::new();
        let input = serde_json::json!({
            "action": "navigate",
            "url": "https://example.com"
        });

        let action = tool.parse_action(&input).unwrap();
        match action {
            BrowserAction::Navigate { url, .. } => {
                assert_eq!(url, "https://example.com");
            }
            other => unreachable!("Expected Navigate action, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_click_action() {
        let tool = BrowserTool::new();
        let input = serde_json::json!({
            "action": "click",
            "selector": "#submit-button"
        });

        let action = tool.parse_action(&input).unwrap();
        match action {
            BrowserAction::Click { selector, .. } => {
                assert_eq!(selector, "#submit-button");
            }
            other => unreachable!("Expected Click action, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_invalid_action() {
        let tool = BrowserTool::new();
        let input = serde_json::json!({
            "action": "invalid_action"
        });

        let result = tool.parse_action(&input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_required_param() {
        let tool = BrowserTool::new();

        // Missing selector for click
        let input = serde_json::json!({
            "action": "click"
        });
        let result = tool.parse_action(&input);
        assert!(result.is_err());

        // Missing url for navigate
        let input = serde_json::json!({
            "action": "navigate"
        });
        let result = tool.parse_action(&input);
        assert!(result.is_err());
    }

    #[test]
    fn test_browser_config_integration() {
        let config = BrowserConfig {
            enabled: false,
            ..Default::default()
        };
        let tool = BrowserTool::with_config(config);
        assert!(!tool.config.enabled);
    }
}
