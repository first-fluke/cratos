//! Browser Automation Module
//!
//! This module provides browser automation capabilities through MCP servers.
//! It supports Playwright and Puppeteer backends via the Model Context Protocol.
//!
//! ## Features
//!
//! - Navigate to URLs
//! - Click, type, fill form elements
//! - Take screenshots
//! - Extract text and HTML content
//! - Wait for elements or navigation
//! - Execute JavaScript
//! - Handle forms (select, checkbox)
//! - Keyboard and scroll interactions
//!
//! ## Usage
//!
//! ```no_run
//! use cratos_tools::browser::{BrowserTool, BrowserConfig};
//! use cratos_tools::registry::Tool;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let tool = BrowserTool::new();
//!
//! // Navigate to a page
//! let result = tool.execute(serde_json::json!({
//!     "action": "navigate",
//!     "url": "https://example.com"
//! })).await?;
//!
//! // Click a button
//! let result = tool.execute(serde_json::json!({
//!     "action": "click",
//!     "selector": "#submit-button"
//! })).await?;
//!
//! // Take a screenshot
//! let result = tool.execute(serde_json::json!({
//!     "action": "screenshot",
//!     "full_page": true
//! })).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Configuration
//!
//! The browser tool can be configured via `config/default.toml`:
//!
//! ```toml
//! [browser]
//! enabled = true
//! default_engine = "playwright"
//!
//! [browser.playwright]
//! browser_type = "chromium"
//! headless = true
//! timeout = 30000
//! ```
//!
//! ## MCP Server Requirements
//!
//! This tool requires an MCP-compatible browser server:
//!
//! - **Playwright**: `npm install -g @anthropic-ai/mcp-server-playwright`
//! - **Puppeteer**: `npm install -g @anthropic-ai/mcp-server-puppeteer`

mod actions;
mod config;
mod tool;

// ============================================================================
// Browser Constants
// ============================================================================

/// Default browser action timeout in milliseconds (120 seconds)
pub const DEFAULT_BROWSER_TIMEOUT_MS: u64 = 120000;

pub use actions::{BrowserAction, BrowserActionResult};
pub use config::{BrowserConfig, BrowserEngine, BrowserType, PlaywrightConfig};
pub use tool::BrowserTool;
