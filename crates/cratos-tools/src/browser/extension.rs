use tracing::{info, warn, debug};
use crate::error::{Error, Result};
use super::tool::BrowserTool;
use super::actions::{BrowserAction, BrowserActionResult};

impl BrowserTool {
    /// Check if the browser extension is connected (via REST status endpoint).
    pub(super) async fn is_extension_connected(&self) -> bool {
        let url = format!("{}/api/v1/browser/status", self.config.server_url);
        debug!(url = %url, backend = ?self.config.backend, "Checking extension connection");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(500))
            .build();
        let client = match client {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Failed to build HTTP client for extension check");
                return false;
            }
        };
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    let connected = body
                        .get("connected")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    info!(connected = connected, url = %self.config.server_url, "Extension status check result");
                    connected
                } else {
                    warn!("Extension status response not parseable");
                    false
                }
            }
            Err(e) => {
                warn!(error = %e, url = %url, "Extension status check failed");
                false
            }
        }
    }

    /// Execute get_tabs via extension relay REST API.
    pub(super) async fn execute_get_tabs(&self) -> Result<BrowserActionResult> {
        let url = format!("{}/api/v1/browser/tabs", self.config.server_url);
        info!("Fetching browser tabs via extension relay");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| Error::Execution(format!("HTTP client error: {}", e)))?;

        match client.get(&url).send().await {
            Ok(resp) => {
                let body: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| Error::Execution(format!("Tab list response error: {}", e)))?;
                if let Some(err) = body.get("error").and_then(|v| v.as_str()) {
                    return Ok(BrowserActionResult::failure(err));
                }
                Ok(BrowserActionResult::success(body))
            }
            Err(e) => Ok(BrowserActionResult::failure(format!(
                "Browser extension not connected. Cannot list tabs. Error: {}",
                e
            ))),
        }
    }

    /// Execute via Chrome extension relay (REST API).
    pub(super) async fn execute_via_extension(&self, action: BrowserAction) -> Result<BrowserActionResult> {
        // Route to correct REST endpoint based on action type
        let (endpoint, params) = match &action {
            BrowserAction::Navigate { url, .. } => {
                ("/api/v1/browser/open", serde_json::json!({ "url": url }))
            }
            BrowserAction::Screenshot { .. } => {
                ("/api/v1/browser/screenshot", action.to_relay_args())
            }
            _ => ("/api/v1/browser/action", action.to_relay_args()),
        };
        let url = format!("{}{}", self.config.server_url, endpoint);

        info!(action = ?action.name(), endpoint = endpoint, "Executing browser action via extension relay");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Execution(format!("HTTP client error: {}", e)))?;

        let resp = client
            .post(&url)
            .json(&params)
            .send()
            .await
            .map_err(|e| Error::Execution(format!("Extension relay request failed: {}", e)))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Error::Execution(format!("Extension relay response error: {}", e)))?;

        if let Some(err) = body.get("error").and_then(|v| v.as_str()) {
            return Ok(BrowserActionResult::failure(err));
        }

        let screenshot = body
            .get("screenshot")
            .and_then(|v| v.as_str())
            .map(String::from);
        let mut result = BrowserActionResult::success(body);
        if let Some(ss) = screenshot {
            result = result.with_screenshot(ss);
        }
        Ok(result)
    }
}

/// Returns true if the error is a DOM/page-level issue that would fail
/// identically on any browser backend (extension or MCP).
/// These errors should NOT trigger a fallback to MCP.
pub(super) fn is_dom_level_error(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("element not found")
        || lower.contains("not found:")
        || lower.contains("cannot find")
        || lower.contains("no element")
        || lower.contains("restricted page")
        || lower.contains("action not supported")
        || lower.contains("cannot read properties")
        || lower.contains("is not defined")
        || lower.contains("syntax error")
        || lower.contains("evaluation failed")
        || lower.contains("not a valid selector")
}
