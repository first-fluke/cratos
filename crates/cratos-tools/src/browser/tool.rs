//! Browser tool implementation
//!
//! Provides a unified browser automation tool that wraps MCP browser servers.

use crate::error::Result;
use crate::mcp::McpClient;
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::actions::{BrowserAction, BrowserActionResult};
use super::config::{BrowserBackend, BrowserConfig};
use super::extension::is_dom_level_error;

/// Browser automation tool
///
/// This tool provides browser automation capabilities by connecting to
/// MCP browser servers (Playwright or Puppeteer).
pub struct BrowserTool {
    pub(crate) definition: ToolDefinition,
    pub(crate) config: BrowserConfig,
    pub(crate) mcp_client: Arc<RwLock<Option<McpClient>>>,
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
            "Control the user's real web browser. Capabilities: search sites (search), click by visible text (click_text — preferred over click when you don't know CSS selectors), list open tabs (get_tabs), navigate, click by CSS selector, fill forms, take screenshots, read page text/HTML, execute JavaScript. Use click_text when you can see text on the page but don't know the CSS selector.",
        )
        .with_category(ToolCategory::External)
        .with_risk_level(RiskLevel::Medium)
        .with_capability("browser")
        .with_parameters(Self::build_parameters_schema());

        Self {
            definition,
            config,
            mcp_client: Arc::new(RwLock::new(None)),
        }
    }

    fn build_parameters_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Action to perform. Use 'search' to search on a known site. Use 'click_text' to click an element by its visible text (no CSS selector needed).",
                    "enum": [
                        "search", "click_text",
                        "get_tabs", "navigate", "click", "type", "fill", "screenshot",
                        "get_text", "get_html", "get_attribute",
                        "wait_for_selector", "wait_for_navigation",
                        "evaluate", "select", "check", "hover", "press", "scroll",
                        "get_url", "get_title", "go_back", "go_forward", "reload", "close"
                    ]
                },
                "site": {
                    "type": "string",
                    "description": "Site to search on (for search action): naver_shopping, naver, coupang, google, youtube, amazon, google_maps"
                },
                "query": {
                    "type": "string",
                    "description": "Search query text (for search action)"
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (for navigate action)"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for element actions (required for click, type, fill; optional for get_text — omit to read entire page)"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type (for type action), or visible text to find and click (for click_text action)"
                },
                "index": {
                    "type": "integer",
                    "description": "Which match to click for click_text (0=first match, default: 0)"
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
                "delay": {
                    "type": "integer",
                    "description": "Delay between keystrokes in ms for type action (default: 50)"
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
        })
    }

    /// Execute a browser action using the configured backend.
    async fn execute_action(&self, action: BrowserAction) -> Result<BrowserActionResult> {
        // Detect search action before resolving
        let is_search = matches!(action, BrowserAction::Search { .. });

        // Resolve Search → Navigate with the correct search URL
        let action = action.resolve_search();

        // GetTabs is extension-only (Chrome tabs API has no MCP equivalent)
        if matches!(action, BrowserAction::GetTabs) {
            return self.execute_get_tabs().await;
        }

        let result = match self.config.backend {
            BrowserBackend::Mcp => self.execute_via_mcp(action).await?,
            BrowserBackend::Extension => self.execute_via_extension(action).await?,
            BrowserBackend::Auto => self.execute_with_fallback(action).await?,
        };

        // After a successful search navigate, auto-read page text so the LLM
        // can see the search results without a separate get_text call.
        if is_search && result.success {
            // Small delay for page JS to render search results
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

            let text_action = BrowserAction::GetText { selector: None };
            let text_result = match self.config.backend {
                BrowserBackend::Mcp => self.execute_via_mcp(text_action).await,
                BrowserBackend::Extension => self.execute_via_extension(text_action).await,
                BrowserBackend::Auto => self.execute_with_fallback(text_action).await,
            };

            if let Ok(tr) = text_result {
                if tr.success {
                    info!("Search auto-read: got page text");
                    return Ok(BrowserActionResult::success(serde_json::json!({
                        "navigated": result.data,
                        "page_text": tr.data
                    })));
                }
            }
        }

        Ok(result)
    }

    /// Execute a browser action with automatic fallback (Extension -> MCP).
    async fn execute_with_fallback(&self, action: BrowserAction) -> Result<BrowserActionResult> {
        if !self.is_extension_connected().await {
            return self.execute_via_mcp(action).await;
        }

        info!("Extension connected, trying extension relay first");
        match self.execute_via_extension(action.clone()).await {
            Ok(result) if result.success => {
                info!("Extension relay succeeded");
                Ok(result)
            }
            Ok(result) => {
                let error_msg = result.error.as_deref().unwrap_or("");
                if is_dom_level_error(error_msg) {
                    // DOM/page error: MCP would fail identically, return directly
                    warn!(error = ?result.error, "Extension DOM error, no fallback");
                    Ok(result)
                } else {
                    // Infrastructure error: try MCP fallback
                    warn!(error = ?result.error, "Extension infra error, falling back to MCP");
                    match self.execute_via_mcp(action).await {
                        Ok(mcp_result) => Ok(mcp_result),
                        Err(_) => Ok(result),
                    }
                }
            }
            Err(e) => {
                // Transport error (HTTP request failed): fall back to MCP
                warn!(error = %e, "Extension relay error, falling back to MCP");
                self.execute_via_mcp(action).await
            }
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
