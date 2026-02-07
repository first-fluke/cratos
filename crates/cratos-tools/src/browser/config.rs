//! Browser tool configuration
//!
//! Configuration types for the browser automation tool.

use serde::{Deserialize, Serialize};

/// Browser engine type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserEngine {
    /// Playwright MCP server (recommended)
    #[default]
    Playwright,
    /// Puppeteer MCP server
    Puppeteer,
}

impl BrowserEngine {
    /// Get the default MCP server command for this engine
    #[must_use]
    pub fn mcp_command(&self) -> (&'static str, Vec<&'static str>) {
        match self {
            Self::Playwright => ("npx", vec!["-y", "@playwright/mcp", "--stdio"]),
            Self::Puppeteer => ("npx", vec!["-y", "@anthropic-ai/mcp-server-puppeteer"]),
        }
    }

    /// Get the server name for MCP client
    #[must_use]
    pub fn server_name(&self) -> &'static str {
        match self {
            Self::Playwright => "playwright",
            Self::Puppeteer => "puppeteer",
        }
    }
}

/// Browser type for Playwright
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserType {
    /// Chromium browser (default)
    #[default]
    Chromium,
    /// Firefox browser
    Firefox,
    /// WebKit browser
    Webkit,
}

/// Browser configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    /// Whether browser tool is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Default browser engine
    #[serde(default)]
    pub default_engine: BrowserEngine,

    /// Playwright-specific configuration
    #[serde(default)]
    pub playwright: PlaywrightConfig,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_engine: BrowserEngine::default(),
            playwright: PlaywrightConfig::default(),
        }
    }
}

/// Playwright-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaywrightConfig {
    /// Browser type to use
    #[serde(default)]
    pub browser_type: BrowserType,

    /// Run browser in headless mode
    #[serde(default = "default_true")]
    pub headless: bool,

    /// Default timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Default viewport width
    #[serde(default = "default_viewport_width")]
    pub viewport_width: u32,

    /// Default viewport height
    #[serde(default = "default_viewport_height")]
    pub viewport_height: u32,
}

impl Default for PlaywrightConfig {
    fn default() -> Self {
        Self {
            browser_type: BrowserType::default(),
            headless: true,
            timeout: default_timeout(),
            viewport_width: default_viewport_width(),
            viewport_height: default_viewport_height(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    super::DEFAULT_BROWSER_TIMEOUT_MS
}

fn default_viewport_width() -> u32 {
    1280
}

fn default_viewport_height() -> u32 {
    720
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_config_default() {
        let config = BrowserConfig::default();
        assert!(config.enabled);
        assert_eq!(config.default_engine, BrowserEngine::Playwright);
        assert!(config.playwright.headless);
        assert_eq!(config.playwright.timeout, 30000);
    }

    #[test]
    fn test_browser_engine_mcp_command() {
        let (cmd, args) = BrowserEngine::Playwright.mcp_command();
        assert_eq!(cmd, "npx");
        assert!(args.contains(&"@playwright/mcp"));
    }

    #[test]
    fn test_browser_config_deserialization() {
        let toml = r#"
            enabled = true
            default_engine = "playwright"

            [playwright]
            browser_type = "chromium"
            headless = false
            timeout = 60000
        "#;

        let config: BrowserConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert!(!config.playwright.headless);
        assert_eq!(config.playwright.timeout, 60000);
    }
}
