use std::collections::HashMap;
use tracing::{info, warn, debug};
use crate::error::{Error, Result};
use crate::mcp::{McpClient, McpServerConfig, McpTransport};
use super::tool::BrowserTool;
use super::actions::{BrowserAction, BrowserActionResult};

impl BrowserTool {
    /// Ensure MCP client is connected
    pub(super) async fn ensure_connected(&self) -> Result<()> {
        let mut client_guard = self.mcp_client.write().await;

        if client_guard.is_some() {
            return Ok(());
        }

        info!("Starting browser MCP server");

        let engine = &self.config.default_engine;
        let (command, args) = engine.mcp_command();

        let mut env: HashMap<String, String> = HashMap::new();
        if self.config.playwright.slow_mo > 0 {
            env.insert(
                "PLAYWRIGHT_SLOW_MO".to_string(),
                self.config.playwright.slow_mo.to_string(),
            );
        }

        let server_config = McpServerConfig {
            name: engine.server_name().to_string(),
            transport: McpTransport::Stdio {
                command: command.to_string(),
                args: args.iter().map(|s| (*s).to_string()).collect(),
                env,
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

    /// Execute via MCP server (Playwright/Puppeteer).
    pub(super) async fn execute_via_mcp(&self, action: BrowserAction) -> Result<BrowserActionResult> {
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
            "Executing browser action via MCP"
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
        let mut screenshot = result.content.iter().find_map(|c| {
            if let crate::mcp::McpContent::Image { data, .. } = c {
                Some(data.clone())
            } else {
                None
            }
        });

        // Fallback: Check if data contains screenshot/image/base64
        if screenshot.is_none() {
            if let Some(s) = data.get("screenshot").and_then(|v| v.as_str()) {
                screenshot = Some(s.to_string());
            } else if let Some(s) = data.get("data").and_then(|v| v.as_str()) {
                // Heuristic: check if it looks like base64
                if s.len() > 100 && !s.contains(' ') {
                    screenshot = Some(s.to_string());
                }
            }
        }

        let mut action_result = BrowserActionResult::success(data);
        if let Some(ss) = screenshot {
            action_result = action_result.with_screenshot(ss);
        }

        Ok(action_result)
    }
}
