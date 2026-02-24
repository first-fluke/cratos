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
            "Control the user's real web browser. Key actions: \
             'search' -- search a known site (REQUIRES site + query, e.g. site=\"naver_shopping\" query=\"keyword\"). Sites: naver, naver_shopping, coupang, google, youtube, amazon, google_maps. \
             'navigate' -- go to a specific URL (REQUIRES url, NOT site). \
             'click_text' -- click element by visible text (REQUIRES 'text' param, e.g. text=\"장바구니\"). Do NOT use 'selector' for click_text. \
             'click' -- click element by CSS selector (REQUIRES 'selector' param). \
             Other: get_tabs, type, fill, screenshot, get_text, get_html, evaluate, scroll, go_back, reload. \
             IMPORTANT: To search a site use 'search' with site+query, NOT 'navigate'. \
             IMPORTANT: click_text needs 'text' param, click needs 'selector' param — do NOT mix them up.",
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
                    "description": "Action to perform. Use 'search' with site+query to search a known site (NOT navigate). Use 'navigate' with url for a specific URL. Use 'click_text' for visible text clicks.",
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
                    "description": "CSS selector (for click, type, fill, hover, check). NOT for click_text — use 'text' param instead. Optional for get_text (omit to read entire page)."
                },
                "text": {
                    "type": "string",
                    "description": "For click_text: the visible text to find and click (e.g. \"장바구니\", \"구매하기\"). For type: the text to type into the element."
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

    /// Dispatch an action to the configured backend.
    async fn dispatch(&self, action: BrowserAction) -> Result<BrowserActionResult> {
        match self.config.backend {
            BrowserBackend::Mcp => self.execute_via_mcp(action).await,
            BrowserBackend::Extension => self.execute_via_extension(action).await,
            BrowserBackend::Auto => self.execute_with_fallback(action).await,
        }
    }

    /// Execute a browser action using the configured backend.
    async fn execute_action(&self, action: BrowserAction) -> Result<BrowserActionResult> {
        // ClickText: two-phase (find element → navigate or click)
        if let BrowserAction::ClickText { ref text, index } = action {
            return self.execute_click_text(text, index).await;
        }

        // Detect search action before resolving
        let is_search = matches!(action, BrowserAction::Search { .. });

        // Resolve Search → Navigate with the correct search URL
        let action = action.resolve_search();

        // GetTabs is extension-only (Chrome tabs API has no MCP equivalent)
        if matches!(action, BrowserAction::GetTabs) {
            return self.execute_get_tabs().await;
        }

        let result = self.dispatch(action).await?;

        // After a successful search navigate, auto-read page text so the LLM
        // can see the search results without a separate get_text call.
        if is_search && result.success {
            // Small delay for page JS to render search results
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

            let text_action = BrowserAction::GetText { selector: None };
            let text_result = self.dispatch(text_action).await;

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

    /// Two-phase click_text: find element via JS, then navigate or click.
    ///
    /// Phase 1: Evaluate JS that finds the matching element and returns structured
    /// info: `{type: "navigate", url, label}` for links, `{type: "clicked", label}`
    /// for buttons/non-link elements. For links, the JS does NOT navigate — it only
    /// identifies the element. For non-links, `target.click()` is called by the JS.
    ///
    /// Phase 2a (link found): Send a Navigate action (which waits for page load).
    /// Phase 2b (clicked, no link): Wait 2s, check if URL changed (JS click may
    /// have triggered navigation). If so, auto-read the new page.
    async fn execute_click_text(&self, text: &str, index: u32) -> Result<BrowserActionResult> {
        // Get current URL before clicking (for navigation detection in Phase 2b)
        let url_before = self.get_current_url().await;

        // Phase 1: Find element and click/identify
        let find_action = BrowserAction::ClickText {
            text: text.to_string(),
            index,
        };
        let result = self.dispatch(find_action).await?;

        if !result.success {
            return Ok(result);
        }

        // Parse the structured result from the JS evaluate.
        // Extension response format: { "result": { type, url?, label, matchInfo } }
        info!(data = %result.data, "click_text Phase 1 raw response");
        let info = Self::extract_click_text_info(&result.data);

        if let Some(info) = info {
            let label = info
                .get("label")
                .and_then(|l| l.as_str())
                .unwrap_or("");
            let match_info = info
                .get("matchInfo")
                .and_then(|m| m.as_str())
                .unwrap_or("");

            if info.get("type").and_then(|t| t.as_str()) == Some("navigate") {
                if let Some(url) = info.get("url").and_then(|u| u.as_str()) {
                    info!(
                        url = url,
                        label = label,
                        "click_text: link detected, navigating with page load wait"
                    );

                    // Phase 2a: Navigate (waits for page load in the extension)
                    let nav_action = BrowserAction::Navigate {
                        url: url.to_string(),
                        wait_until_loaded: true,
                    };
                    let nav_result = self.dispatch(nav_action).await?;

                    if nav_result.success {
                        // Auto-read page text after navigation
                        let page_text = self.auto_read_page_text().await;
                        return Ok(BrowserActionResult::success(serde_json::json!({
                            "clicked": label,
                            "navigated_to": url,
                            "match_info": match_info,
                            "page_loaded": true,
                            "page_text": page_text,
                        })));
                    } else {
                        return Ok(nav_result);
                    }
                }
            }

            // Phase 2b: type === "clicked" — element was clicked by JS directly.
            // The click may have triggered JS-based navigation (onclick handlers).
            // Wait and check if the URL changed.
            info!(label = label, "click_text: element clicked directly, checking for navigation");
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

            let url_after = self.get_current_url().await;

            let navigated = match (&url_before, &url_after) {
                (Some(before), Some(after)) => before != after,
                _ => false,
            };

            if navigated {
                let new_url = url_after.as_deref().unwrap_or("unknown");
                info!(
                    old_url = url_before.as_deref().unwrap_or("?"),
                    new_url = new_url,
                    "click_text: detected navigation after click"
                );
                // Auto-read page text after navigation
                let page_text = self.auto_read_page_text().await;
                return Ok(BrowserActionResult::success(serde_json::json!({
                    "clicked": label,
                    "navigated_to": new_url,
                    "match_info": match_info,
                    "page_loaded": true,
                    "page_text": page_text,
                })));
            }

            return Ok(BrowserActionResult::success(serde_json::json!({
                "clicked": label,
                "match_info": match_info,
            })));
        }

        // Could not parse structured info — return raw result
        Ok(result)
    }

    /// Get the current page URL, returning None if unavailable.
    async fn get_current_url(&self) -> Option<String> {
        let url_action = BrowserAction::GetUrl;
        match self.dispatch(url_action).await {
            Ok(r) if r.success => {
                // Extension returns {"result": "https://..."}, MCP returns direct string
                r.data
                    .get("result")
                    .and_then(|v| v.as_str())
                    .or_else(|| r.data.as_str())
                    .map(String::from)
            }
            _ => None,
        }
    }

    /// Auto-read page text (truncated), returning the text or null.
    async fn auto_read_page_text(&self) -> serde_json::Value {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let text_action = BrowserAction::GetText { selector: None };
        match self.dispatch(text_action).await {
            Ok(r) if r.success => {
                info!("click_text auto-read: got page text");
                r.data
            }
            _ => serde_json::Value::Null,
        }
    }

    /// Extract the structured click_text info from various response formats.
    fn extract_click_text_info(data: &serde_json::Value) -> Option<serde_json::Value> {
        // Extension evaluate returns: { "result": <JS return value> }
        if let Some(r) = data.get("result") {
            if r.get("type").is_some() {
                return Some(r.clone());
            }
            // result might be a JSON string
            if let Some(s) = r.as_str() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
                    if v.get("type").is_some() {
                        return Some(v);
                    }
                }
            }
        }
        // data itself has "type" field (MCP might return directly)
        if data.get("type").is_some() {
            return Some(data.clone());
        }
        // data is a JSON string
        if let Some(s) = data.as_str() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
                if v.get("type").is_some() {
                    return Some(v);
                }
            }
        }
        None
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

        // Parse the action — on failure, capture a screenshot so the LLM can
        // visually inspect the current page state alongside the error message.
        let action = match self.parse_action(&input) {
            Ok(a) => a,
            Err(e) => {
                let mut screenshot: Option<String> = None;
                if let Ok(ss_result) = self
                    .dispatch(BrowserAction::Screenshot {
                        path: None,
                        full_page: false,
                        selector: None,
                    })
                    .await
                {
                    if let Some(ss) = ss_result.screenshot {
                        info!("Auto-captured screenshot for browser parse error");
                        screenshot = Some(ss);
                    }
                }
                let duration = start.elapsed().as_millis() as u64;
                return Ok(ToolResult {
                    success: false,
                    output: serde_json::json!({
                        "error": e.to_string(),
                        "screenshot": screenshot
                    }),
                    error: Some(e.to_string()),
                    duration_ms: duration,
                });
            }
        };
        let is_interactive = action.is_interactive();
        debug!(action = ?action.name(), "Executing browser action");

        // Execute the action
        let mut result = self.execute_action(action).await?;

        // Auto-capture screenshot on interactive action failure so the LLM can
        // visually inspect the page and decide the correct next step.
        if !result.success && is_interactive && result.screenshot.is_none() {
            if let Ok(ss_result) = self
                .dispatch(BrowserAction::Screenshot {
                    path: None,
                    full_page: false,
                    selector: None,
                })
                .await
            {
                if let Some(ss) = ss_result.screenshot {
                    info!("Auto-captured screenshot for failed browser action");
                    result = result.with_screenshot(ss);
                }
            }
        }

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
                    "error": result.error,
                    "screenshot": result.screenshot
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
