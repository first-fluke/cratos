//! Browser actions
//!
//! Defines the actions that can be performed by the browser tool.

use serde::{Deserialize, Serialize};

/// Browser action types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserAction {
    /// Navigate to a URL
    Navigate {
        /// URL to navigate to
        url: String,
        /// Wait for navigation to complete (default: true)
        #[serde(default = "default_true")]
        wait_until_loaded: bool,
    },

    /// Click on an element
    Click {
        /// CSS selector for the element
        selector: String,
        /// Optional button (left, right, middle)
        #[serde(default)]
        button: Option<String>,
    },

    /// Type text into an element
    Type {
        /// CSS selector for the element
        selector: String,
        /// Text to type
        text: String,
        /// Delay between keystrokes in milliseconds
        #[serde(default)]
        delay: Option<u64>,
    },

    /// Fill a form field (clears existing content first)
    Fill {
        /// CSS selector for the element
        selector: String,
        /// Value to fill
        value: String,
    },

    /// Take a screenshot
    Screenshot {
        /// Path to save the screenshot (optional)
        #[serde(default)]
        path: Option<String>,
        /// Whether to capture full page (default: false)
        #[serde(default)]
        full_page: bool,
        /// CSS selector to screenshot a specific element
        #[serde(default)]
        selector: Option<String>,
    },

    /// Get text content of an element
    GetText {
        /// CSS selector for the element
        selector: String,
    },

    /// Get HTML content of an element
    GetHtml {
        /// CSS selector for the element
        selector: String,
        /// Whether to get outer HTML (default: true)
        #[serde(default = "default_true")]
        outer: bool,
    },

    /// Get attribute value of an element
    GetAttribute {
        /// CSS selector for the element
        selector: String,
        /// Attribute name
        attribute: String,
    },

    /// Wait for an element to appear
    WaitForSelector {
        /// CSS selector to wait for
        selector: String,
        /// Timeout in milliseconds
        #[serde(default = "default_timeout")]
        timeout: u64,
        /// Wait for element to be visible (default: true)
        #[serde(default = "default_true")]
        visible: bool,
    },

    /// Wait for navigation to complete
    WaitForNavigation {
        /// Timeout in milliseconds
        #[serde(default = "default_timeout")]
        timeout: u64,
    },

    /// Evaluate JavaScript in the browser
    Evaluate {
        /// JavaScript code to evaluate
        script: String,
    },

    /// Select an option from a dropdown
    Select {
        /// CSS selector for the select element
        selector: String,
        /// Value to select
        value: String,
    },

    /// Check or uncheck a checkbox
    Check {
        /// CSS selector for the checkbox
        selector: String,
        /// Whether to check (true) or uncheck (false)
        #[serde(default = "default_true")]
        checked: bool,
    },

    /// Hover over an element
    Hover {
        /// CSS selector for the element
        selector: String,
    },

    /// Press a keyboard key
    Press {
        /// Key to press (e.g., "Enter", "Tab", "Escape")
        key: String,
        /// Number of times to press
        #[serde(default = "default_one")]
        count: u32,
    },

    /// Scroll the page or element
    Scroll {
        /// CSS selector for the element to scroll (optional, scrolls page if not provided)
        #[serde(default)]
        selector: Option<String>,
        /// Horizontal scroll amount in pixels
        #[serde(default)]
        x: i32,
        /// Vertical scroll amount in pixels
        #[serde(default)]
        y: i32,
    },

    /// Get the current URL
    GetUrl,

    /// Get the page title
    GetTitle,

    /// Go back in browser history
    GoBack,

    /// Go forward in browser history
    GoForward,

    /// Reload the page
    Reload,

    /// Close the browser
    Close,
}

impl BrowserAction {
    /// Get the action name
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Navigate { .. } => "navigate",
            Self::Click { .. } => "click",
            Self::Type { .. } => "type",
            Self::Fill { .. } => "fill",
            Self::Screenshot { .. } => "screenshot",
            Self::GetText { .. } => "get_text",
            Self::GetHtml { .. } => "get_html",
            Self::GetAttribute { .. } => "get_attribute",
            Self::WaitForSelector { .. } => "wait_for_selector",
            Self::WaitForNavigation { .. } => "wait_for_navigation",
            Self::Evaluate { .. } => "evaluate",
            Self::Select { .. } => "select",
            Self::Check { .. } => "check",
            Self::Hover { .. } => "hover",
            Self::Press { .. } => "press",
            Self::Scroll { .. } => "scroll",
            Self::GetUrl => "get_url",
            Self::GetTitle => "get_title",
            Self::GoBack => "go_back",
            Self::GoForward => "go_forward",
            Self::Reload => "reload",
            Self::Close => "close",
        }
    }

    /// Convert to MCP tool call arguments
    #[must_use]
    pub fn to_mcp_args(&self) -> serde_json::Value {
        // For Playwright MCP server, we map actions to their tool names and arguments
        match self {
            Self::Navigate { url, .. } => {
                serde_json::json!({
                    "url": url
                })
            }
            Self::Click { selector, button } => {
                let mut args = serde_json::json!({
                    "selector": selector
                });
                if let Some(btn) = button {
                    args["button"] = serde_json::Value::String(btn.clone());
                }
                args
            }
            Self::Type {
                selector,
                text,
                delay,
            } => {
                let mut args = serde_json::json!({
                    "selector": selector,
                    "text": text
                });
                if let Some(d) = delay {
                    args["delay"] = serde_json::json!(d);
                }
                args
            }
            Self::Fill { selector, value } => {
                serde_json::json!({
                    "selector": selector,
                    "value": value
                })
            }
            Self::Screenshot {
                path,
                full_page,
                selector,
            } => {
                let mut args = serde_json::json!({
                    "fullPage": full_page
                });
                if let Some(p) = path {
                    args["path"] = serde_json::Value::String(p.clone());
                }
                if let Some(s) = selector {
                    args["selector"] = serde_json::Value::String(s.clone());
                }
                args
            }
            Self::GetText { selector } => {
                serde_json::json!({
                    "selector": selector
                })
            }
            Self::GetHtml { selector, outer } => {
                serde_json::json!({
                    "selector": selector,
                    "outer": outer
                })
            }
            Self::GetAttribute {
                selector,
                attribute,
            } => {
                serde_json::json!({
                    "selector": selector,
                    "attribute": attribute
                })
            }
            Self::WaitForSelector {
                selector,
                timeout,
                visible,
            } => {
                serde_json::json!({
                    "selector": selector,
                    "timeout": timeout,
                    "visible": visible
                })
            }
            Self::WaitForNavigation { timeout } => {
                serde_json::json!({
                    "timeout": timeout
                })
            }
            Self::Evaluate { script } => {
                serde_json::json!({
                    "script": script
                })
            }
            Self::Select { selector, value } => {
                serde_json::json!({
                    "selector": selector,
                    "value": value
                })
            }
            Self::Check { selector, checked } => {
                serde_json::json!({
                    "selector": selector,
                    "checked": checked
                })
            }
            Self::Hover { selector } => {
                serde_json::json!({
                    "selector": selector
                })
            }
            Self::Press { key, count } => {
                serde_json::json!({
                    "key": key,
                    "count": count
                })
            }
            Self::Scroll { selector, x, y } => {
                let mut args = serde_json::json!({
                    "x": x,
                    "y": y
                });
                if let Some(s) = selector {
                    args["selector"] = serde_json::Value::String(s.clone());
                }
                args
            }
            Self::GetUrl
            | Self::GetTitle
            | Self::GoBack
            | Self::GoForward
            | Self::Reload
            | Self::Close => {
                serde_json::json!({})
            }
        }
    }

    /// Get the MCP tool name for this action
    #[must_use]
    pub fn mcp_tool_name(&self) -> &'static str {
        match self {
            Self::Navigate { .. } => "browser_navigate",
            Self::Click { .. } => "browser_click",
            Self::Type { .. } => "browser_type",
            Self::Fill { .. } => "browser_fill",
            Self::Screenshot { .. } => "browser_screenshot",
            Self::GetText { .. } => "browser_get_text",
            Self::GetHtml { .. } => "browser_get_html",
            Self::GetAttribute { .. } => "browser_get_attribute",
            Self::WaitForSelector { .. } => "browser_wait_for_selector",
            Self::WaitForNavigation { .. } => "browser_wait_for_navigation",
            Self::Evaluate { .. } => "browser_evaluate",
            Self::Select { .. } => "browser_select",
            Self::Check { .. } => "browser_check",
            Self::Hover { .. } => "browser_hover",
            Self::Press { .. } => "browser_press",
            Self::Scroll { .. } => "browser_scroll",
            Self::GetUrl => "browser_get_url",
            Self::GetTitle => "browser_get_title",
            Self::GoBack => "browser_go_back",
            Self::GoForward => "browser_go_forward",
            Self::Reload => "browser_reload",
            Self::Close => "browser_close",
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    super::DEFAULT_BROWSER_TIMEOUT_MS
}

fn default_one() -> u32 {
    1
}

/// Result of a browser action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserActionResult {
    /// Whether the action succeeded
    pub success: bool,
    /// Result data (depends on action type)
    #[serde(default)]
    pub data: serde_json::Value,
    /// Error message if failed
    #[serde(default)]
    pub error: Option<String>,
    /// Screenshot data (base64 encoded) if applicable
    #[serde(default)]
    pub screenshot: Option<String>,
}

impl BrowserActionResult {
    /// Create a successful result
    #[must_use]
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data,
            error: None,
            screenshot: None,
        }
    }

    /// Create a failed result
    #[must_use]
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: serde_json::Value::Null,
            error: Some(error.into()),
            screenshot: None,
        }
    }

    /// Add screenshot data
    #[must_use]
    pub fn with_screenshot(mut self, screenshot: String) -> Self {
        self.screenshot = Some(screenshot);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_action_navigate() {
        let action = BrowserAction::Navigate {
            url: "https://example.com".to_string(),
            wait_until_loaded: true,
        };
        assert_eq!(action.name(), "navigate");
        assert_eq!(action.mcp_tool_name(), "browser_navigate");
    }

    #[test]
    fn test_browser_action_deserialization() {
        let json = r#"{"action": "navigate", "url": "https://example.com"}"#;
        let action: BrowserAction = serde_json::from_str(json).unwrap();
        match action {
            BrowserAction::Navigate { url, .. } => {
                assert_eq!(url, "https://example.com");
            }
            other => unreachable!("Expected Navigate action, got {:?}", other),
        }
    }

    #[test]
    fn test_browser_action_to_mcp_args() {
        let action = BrowserAction::Click {
            selector: "#submit".to_string(),
            button: Some("left".to_string()),
        };
        let args = action.to_mcp_args();
        assert_eq!(args["selector"], "#submit");
        assert_eq!(args["button"], "left");
    }

    #[test]
    fn test_browser_action_result() {
        let result = BrowserActionResult::success(serde_json::json!({"text": "Hello"}));
        assert!(result.success);
        assert!(result.error.is_none());

        let result = BrowserActionResult::failure("Element not found");
        assert!(!result.success);
        assert_eq!(result.error, Some("Element not found".to_string()));
    }
}
