//! MCP Tool Bridge — wraps MCP server tools as native `Tool` trait objects
//!
//! Each `McpToolBridge` represents one remote MCP tool and delegates
//! `execute()` to the shared [`McpClient`] via JSON-RPC.

use crate::error::{Error, Result};
use crate::mcp::protocol::McpContent;
use crate::registry::{Tool, ToolDefinition, ToolResult};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::debug;

use super::McpClient;

/// A single MCP tool exposed through the native `Tool` trait.
pub struct McpToolBridge {
    definition: ToolDefinition,
    server_name: String,
    tool_name: String,
    client: Arc<RwLock<McpClient>>,
}

impl McpToolBridge {
    /// Create a new bridge for the given MCP tool.
    pub fn new(
        definition: ToolDefinition,
        server_name: String,
        tool_name: String,
        client: Arc<RwLock<McpClient>>,
    ) -> Self {
        Self {
            definition,
            server_name,
            tool_name,
            client,
        }
    }
}

#[async_trait::async_trait]
impl Tool for McpToolBridge {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        debug!(
            server = %self.server_name,
            tool = %self.tool_name,
            "Calling MCP tool"
        );

        let client = self.client.read().await;
        let mcp_result = client
            .call_tool(&self.server_name, &self.tool_name, input)
            .await
            .map_err(|e| Error::Execution(format!("MCP call failed: {}", e)))?;

        let duration = start.elapsed().as_millis() as u64;

        // Concatenate all text content from the MCP response
        let text: String = mcp_result
            .content
            .iter()
            .filter_map(|c| match c {
                McpContent::Text { text } => Some(text.as_str()),
                McpContent::Resource { text: Some(t), .. } => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        if mcp_result.is_error {
            Ok(ToolResult::failure(text, duration))
        } else {
            Ok(ToolResult::success(serde_json::json!({ "output": text }), duration))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{RiskLevel, ToolCategory};

    #[test]
    fn test_bridge_definition() {
        let def = ToolDefinition::new("mcp_test_echo", "Echo tool")
            .with_category(ToolCategory::External)
            .with_risk_level(RiskLevel::Medium);

        let client = Arc::new(RwLock::new(McpClient::new()));
        let bridge = McpToolBridge::new(def, "test".into(), "echo".into(), client);

        assert_eq!(bridge.definition().name, "mcp_test_echo");
        assert_eq!(bridge.definition().category, ToolCategory::External);
    }
}
