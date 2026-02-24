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

    /// Get text content of an element (or the whole page if no selector)
    GetText {
        /// CSS selector for the element (optional, defaults to body)
        #[serde(default)]
        selector: Option<String>,
    },

    /// Get HTML content of an element
    GetHtml {
        /// CSS selector for the element (optional, defaults to "html")
        #[serde(default)]
        selector: Option<String>,
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

    /// Resize the browser window
    Resize {
        /// Width in pixels
        width: u32,
        /// Height in pixels
        height: u32,
    },

    /// Click an element by its visible text content
    ClickText {
        /// Text to find and click (partial match, case-insensitive)
        text: String,
        /// Which match to click (0-indexed, default: 0 = first match)
        #[serde(default)]
        index: u32,
    },

    /// Search on a known site (auto-constructs the search URL)
    Search {
        /// Site identifier: "naver_shopping", "naver", "coupang", "google",
        /// "youtube", "amazon", "google_maps"
        site: String,
        /// Search query
        query: String,
    },

    /// List all open browser tabs (extension relay only)
    GetTabs,

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
            Self::Resize { .. } => "resize",
            Self::ClickText { .. } => "click_text",
            Self::Search { .. } => "search",
            Self::GetTabs => "get_tabs",
            Self::Close => "close",
        }
    }

    /// Whether this action is an interactive page mutation (click, navigate, fill, etc.)
    /// as opposed to a read-only query (get_text, get_url, screenshot).
    /// Used to decide whether to auto-capture a screenshot on failure.
    #[must_use]
    pub fn is_interactive(&self) -> bool {
        matches!(
            self,
            Self::Navigate { .. }
                | Self::Click { .. }
                | Self::ClickText { .. }
                | Self::Type { .. }
                | Self::Fill { .. }
                | Self::Search { .. }
                | Self::Select { .. }
                | Self::Check { .. }
                | Self::Hover { .. }
                | Self::Press { .. }
                | Self::Scroll { .. }
        )
    }

    /// Build search URL from site identifier and query.
    /// Returns the fully constructed URL, or None if the site is unknown.
    fn build_search_url(site: &str, query: &str) -> Option<String> {
        let q = urlencoding::encode(query);
        match site {
            "naver_shopping" | "naver-shopping" | "네이버쇼핑" => {
                Some(format!("https://search.shopping.naver.com/search/all?query={q}"))
            }
            "naver" | "네이버" => {
                Some(format!("https://search.naver.com/search.naver?query={q}"))
            }
            "coupang" | "쿠팡" => {
                Some(format!("https://www.coupang.com/np/search?q={q}"))
            }
            "google" | "구글" => Some(format!("https://www.google.com/search?q={q}")),
            "youtube" | "유튜브" => {
                Some(format!("https://www.youtube.com/results?search_query={q}"))
            }
            "amazon" | "아마존" => Some(format!("https://www.amazon.com/s?k={q}")),
            "google_maps" | "google-maps" | "구글맵" => {
                Some(format!("https://www.google.com/maps/search/{q}"))
            }
            _ => None,
        }
    }

    /// Resolve Search action into Navigate, falling back to Google search for unknown sites.
    #[must_use]
    pub fn resolve_search(self) -> Self {
        match self {
            Self::Search { ref site, ref query } => {
                let url = Self::build_search_url(site, query).unwrap_or_else(|| {
                    let q = urlencoding::encode(query);
                    let s = urlencoding::encode(site);
                    format!("https://www.google.com/search?q={q}+site:{s}")
                });
                Self::Navigate {
                    url,
                    wait_until_loaded: true,
                }
            }
            other => other,
        }
    }

    /// Convert to relay API parameters (for extension relay mode).
    /// Returns a JSON object with "action" + action-specific fields.
    #[must_use]
    pub fn to_relay_args(&self) -> serde_json::Value {
        match self {
            Self::Navigate { url, .. } => serde_json::json!({ "action": "navigate", "url": url }),
            Self::Click { selector, .. } => {
                serde_json::json!({ "action": "click", "selector": selector })
            }
            Self::Type { selector, text, .. } => {
                serde_json::json!({ "action": "type", "selector": selector, "text": text })
            }
            Self::Fill { selector, value } => {
                serde_json::json!({ "action": "fill", "selector": selector, "value": value })
            }
            Self::Screenshot { selector, .. } => {
                let mut v = serde_json::json!({ "action": "screenshot" });
                if let Some(s) = selector {
                    v["selector"] = serde_json::Value::String(s.clone());
                }
                v
            }
            Self::GetText { selector } => {
                let sel = selector.as_deref().unwrap_or("body");
                serde_json::json!({ "action": "get_text", "selector": sel })
            }
            Self::GetHtml { selector, outer } => {
                serde_json::json!({ "action": "get_html", "selector": selector, "outer": outer })
            }
            Self::GetAttribute {
                selector,
                attribute,
            } => {
                serde_json::json!({ "action": "get_attribute", "selector": selector, "attribute": attribute })
            }
            Self::WaitForSelector {
                selector, timeout, ..
            } => {
                serde_json::json!({ "action": "wait_for_selector", "selector": selector, "timeout": timeout })
            }
            Self::Evaluate { script } => {
                serde_json::json!({ "action": "evaluate", "script": script })
            }
            Self::Select { selector, value } => {
                serde_json::json!({ "action": "select", "selector": selector, "value": value })
            }
            Self::Check { selector, checked } => {
                serde_json::json!({ "action": "check", "selector": selector, "checked": checked })
            }
            Self::Hover { selector } => {
                serde_json::json!({ "action": "hover", "selector": selector })
            }
            Self::Scroll { selector, x, y } => {
                let mut v = serde_json::json!({ "action": "scroll", "x": x, "y": y });
                if let Some(s) = selector {
                    v["selector"] = serde_json::Value::String(s.clone());
                }
                v
            }
            Self::ClickText { .. } => {
                // ClickText is executed via JS evaluate on the extension side
                serde_json::json!({ "action": "evaluate", "script": self.to_js_function() })
            }
            Self::GetUrl => serde_json::json!({ "action": "get_url" }),
            Self::GetTitle => serde_json::json!({ "action": "get_title" }),
            Self::GetTabs => serde_json::json!({ "action": "get_tabs" }),
            _ => serde_json::json!({ "action": self.name() }),
        }
    }

    /// Convert to MCP tool call arguments
    #[must_use]
    pub fn to_mcp_args(&self) -> serde_json::Value {
        match self {
            Self::Navigate { url, .. } => {
                serde_json::json!({
                    "url": url
                })
            }
            Self::Screenshot {
                path,
                full_page,
                selector: _,
            } => {
                let mut args = serde_json::json!({
                    "fullPage": full_page
                });
                if let Some(p) = path {
                    args["filename"] = serde_json::Value::String(p.clone());
                }
                args
            }
            Self::Resize { width, height } => {
                serde_json::json!({
                    "width": width,
                    "height": height
                })
            }
            Self::Close => serde_json::json!({}),

            // All other actions use browser_evaluate with JS
            action => {
                let script = action.to_js_function();
                serde_json::json!({
                    "function": script
                })
            }
        }
    }

    /// Get the MCP tool name for this action
    #[must_use]
    pub fn mcp_tool_name(&self) -> &'static str {
        match self {
            Self::Navigate { .. } => "browser_navigate",
            Self::Screenshot { .. } => "browser_take_screenshot",
            Self::Resize { .. } => "browser_resize",
            Self::Close => "browser_close",
            _ => "browser_evaluate",
        }
    }

    /// Generate JS function for browser_evaluate
    fn to_js_function(&self) -> String {
        match self {
            Self::Click { selector, button } => {
                let sel = serde_json::to_string(selector).expect("serialization failed");
                let btn = match button.as_deref() {
                    Some("right") => 2,
                    Some("middle") => 1,
                    _ => 0,
                };
                format!(
                    "() => {{ const el = document.querySelector({sel}); \
                     if (!el) throw new Error('Element not found: ' + {sel}); \
                     const opts = {{ bubbles: true, cancelable: true, button: {btn} }}; \
                     el.dispatchEvent(new MouseEvent('mousedown', opts)); \
                     el.dispatchEvent(new MouseEvent('mouseup', opts)); \
                     el.dispatchEvent(new MouseEvent('click', opts)); }}",
                    sel = sel,
                    btn = btn
                )
            }
            Self::Type {
                selector,
                text,
                delay,
            } => {
                let sel = serde_json::to_string(selector).expect("serialization failed");
                let val = serde_json::to_string(text).expect("serialization failed");
                let delay_ms = delay.unwrap_or(50);
                format!(
                    "async () => {{ const el = document.querySelector({sel}); \
                     if (!el) throw new Error('Element not found: ' + {sel}); \
                     el.focus(); \
                     const text = {val}; \
                     for (const ch of text) {{ \
                       el.value += ch; \
                       el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
                       el.dispatchEvent(new KeyboardEvent('keypress', {{ key: ch, bubbles: true }})); \
                       await new Promise(r => setTimeout(r, {delay_ms} + Math.random() * 20)); \
                     }} \
                     el.dispatchEvent(new Event('change', {{ bubbles: true }})); }}",
                    sel = sel, val = val, delay_ms = delay_ms
                )
            }
            Self::Fill { selector, value } => {
                let sel = serde_json::to_string(selector).expect("serialization failed");
                let val = serde_json::to_string(value).expect("serialization failed");
                format!(
                    "() => {{ const el = document.querySelector({sel}); \
                     if (!el) throw new Error('Element not found: ' + {sel}); \
                     el.focus(); \
                     el.value = ''; \
                     el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
                     el.value = {val}; \
                     el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
                     el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
                     el.blur(); }}",
                    sel = sel,
                    val = val
                )
            }
            Self::GetText { selector } => {
                let sel_str = selector.as_deref().unwrap_or("body");
                let sel = serde_json::to_string(sel_str).expect("serialization failed");
                // Truncate text to ~8000 chars to avoid token overflow
                format!(
                    "() => {{ const el = document.querySelector({sel}); \
                     if (!el) return null; \
                     const t = el.innerText; \
                     return t.length > 8000 ? t.substring(0, 8000) + '\\n... (truncated)' : t; }}"
                )
            }
            Self::GetHtml { selector, outer } => {
                let selector = selector.as_deref().unwrap_or("html");
                let sel = serde_json::to_string(selector).expect("serialization failed");
                let prop = if *outer { "outerHTML" } else { "innerHTML" };

                // Script to clean and return HTML (prevents huge token usage)
                format!(
                    r#"
                    () => {{
                        const el = document.querySelector({});
                        if (!el) return 'Element not found';
                        const clone = el.cloneNode(true);
                        // Remove non-content elements and interactive cruft
                        clone.querySelectorAll('script, style, svg, link, meta, noscript, iframe').forEach(e => e.remove());
                        let html = clone.{};
                        // Limit to ~15KB (~4k tokens) to avoid 429s
                        if (html.length > 15000) {{
                            return html.substring(0, 15000) + '\n... (truncated)';
                        }}
                        return html;
                    }}
                "#,
                    sel, prop
                )
            }
            Self::GetAttribute {
                selector,
                attribute,
            } => {
                let sel = serde_json::to_string(selector).expect("serialization failed");
                let attr = serde_json::to_string(attribute).expect("serialization failed");
                format!("() => {{ const el = document.querySelector({}); return el ? el.getAttribute({}) : null; }}", sel, attr)
            }
            Self::WaitForSelector {
                selector, timeout, ..
            } => {
                let sel = serde_json::to_string(selector).expect("serialization failed");
                format!(
                    r#"
                    async () => {{
                        const selector = {};
                        const timeout = {};
                        const start = Date.now();
                        while (Date.now() - start < timeout) {{
                            if (document.querySelector(selector)) return true;
                            await new Promise(r => setTimeout(r, 100));
                        }}
                        throw new Error('Timeout waiting for selector: ' + selector);
                    }}
                "#,
                    sel, timeout
                )
            }
            Self::WaitForNavigation { timeout } => {
                // Approximate wait with sleep
                format!(
                    "async () => {{ await new Promise(r => setTimeout(r, {})); }}",
                    timeout
                )
            }
            Self::Evaluate { script } => {
                // Wrap user script in function if not already
                if script.trim().starts_with("()") || script.trim().starts_with("function") {
                    script.clone()
                } else {
                    format!("() => {{ {} }}", script)
                }
            }
            Self::Select { selector, value } => {
                let sel = serde_json::to_string(selector).expect("serialization failed");
                let val = serde_json::to_string(value).expect("serialization failed");
                format!("() => {{ const el = document.querySelector({}); if (!el) throw new Error('Element not found'); el.value = {}; el.dispatchEvent(new Event('change', {{ bubbles: true }})); }}", sel, val)
            }
            Self::Check { selector, checked } => {
                let sel = serde_json::to_string(selector).expect("serialization failed");
                format!("() => {{ const el = document.querySelector({}); if (!el) throw new Error('Element not found'); el.checked = {}; el.dispatchEvent(new Event('change', {{ bubbles: true }})); }}", sel, checked)
            }
            Self::Hover { selector } => {
                let sel = serde_json::to_string(selector).expect("serialization failed");
                format!("() => {{ const el = document.querySelector({}); if (!el) throw new Error('Element not found'); el.dispatchEvent(new MouseEvent('mouseover', {{ bubbles: true }})); }}", sel)
            }
            Self::Press { key, count } => {
                let k = serde_json::to_string(key).expect("serialization failed");
                format!("() => {{ for(let i=0; i<{}; i++) document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {{ key: {}, bubbles: true }})); }}", count, k)
            }
            Self::Scroll { selector, x, y } => {
                if let Some(s) = selector {
                    let sel = serde_json::to_string(s).expect("serialization failed");
                    format!("() => {{ const el = document.querySelector({}); if(el) el.scrollBy({}, {}); else window.scrollBy({}, {}); }}", sel, x, y, x, y)
                } else {
                    format!("() => {{ window.scrollBy({}, {}); }}", x, y)
                }
            }
            Self::ClickText { text, index } => {
                let txt = serde_json::to_string(text).expect("serialization failed");
                let idx = index;
                // Walk clickable elements, find by visible text (partial, case-insensitive).
                // Returns structured JSON: {type: "navigate", url, label, matchInfo} for links,
                // or {type: "clicked", label, matchInfo} for buttons.
                // Rust code handles the actual navigation (waits for page load).
                format!(
                    r#"() => {{
  const query = {txt}.toLowerCase();
  function findAnchorHref(el) {{
    let cur = el;
    for (let i = 0; i < 10 && cur; i++) {{
      if (cur.tagName === 'A' && cur.href) return cur.href;
      cur = cur.parentElement;
    }}
    const child = el.querySelector && el.querySelector('a[href]');
    if (child && child.href) return child.href;
    return null;
  }}
  const clickable = document.querySelectorAll('a, button, [role="button"], [role="link"], [onclick], input[type="submit"], input[type="button"], [tabindex]');
  const matches = [];
  for (const el of clickable) {{
    const t = (el.innerText || el.textContent || el.value || el.getAttribute('aria-label') || '').trim().toLowerCase();
    if (t.includes(query)) matches.push(el);
  }}
  if (matches.length === 0) {{
    const all = document.querySelectorAll('*');
    for (const el of all) {{
      if (el.children.length > 3) continue;
      const t = (el.innerText || el.textContent || '').trim().toLowerCase();
      if (t.includes(query) && t.length < 200) matches.push(el);
    }}
  }}
  if (matches.length === 0) throw new Error('No element found with text: ' + {txt});
  const target = matches[Math.min({idx}, matches.length - 1)];
  target.scrollIntoView({{ block: 'center' }});
  const label = (target.innerText || target.textContent || '').trim().substring(0, 100);
  const matchInfo = '(match ' + (Math.min({idx}, matches.length - 1) + 1) + '/' + matches.length + ')';
  const href = findAnchorHref(target);
  if (href && !href.startsWith('javascript:')) {{
    return {{ type: "navigate", url: href, label: label, matchInfo: matchInfo }};
  }}
  target.click();
  return {{ type: "clicked", label: label, matchInfo: matchInfo }};
}}"#,
                    txt = txt,
                    idx = idx
                )
            }
            Self::GetUrl => "() => window.location.href".to_string(),
            Self::GetTitle => "() => document.title".to_string(),
            Self::GoBack => "() => history.back()".to_string(),
            Self::GoForward => "() => history.forward()".to_string(),
            Self::Reload => "() => location.reload()".to_string(),
            _ => "() => {}".to_string(), // Should not happen for Navigate/Screenshot/Close
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
