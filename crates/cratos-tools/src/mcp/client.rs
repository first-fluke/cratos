//! MCP Client
//!
//! High-level client for managing multiple MCP server connections.

use super::protocol::{
    McpContent, McpError, McpInitResult, McpRequest, McpResult, McpTool, McpToolResult,
};
use super::transport::{McpConnection, McpServerConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// MCP client configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpClientConfig {
    /// Whether MCP is enabled
    #[serde(default)]
    pub enabled: bool,
    /// MCP server configurations
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

/// MCP client for managing multiple server connections
pub struct McpClient {
    /// Active connections by server name
    connections: HashMap<String, McpConnection>,
    /// Cached tools by server name
    tool_cache: HashMap<String, Vec<McpTool>>,
}

impl McpClient {
    /// Create a new MCP client
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            tool_cache: HashMap::new(),
        }
    }

    /// Create from configuration
    pub async fn from_config(config: &McpClientConfig) -> McpResult<Self> {
        let mut client = Self::new();

        if !config.enabled {
            info!("MCP is disabled in configuration");
            return Ok(client);
        }

        for server_config in &config.servers {
            if server_config.auto_start {
                if let Err(e) = client.add_server(server_config.clone()).await {
                    warn!(
                        server = %server_config.name,
                        error = %e,
                        "Failed to start MCP server (continuing without it)"
                    );
                }
            }
        }

        Ok(client)
    }

    /// Add and start a server
    pub async fn add_server(&mut self, config: McpServerConfig) -> McpResult<()> {
        let name = config.name.clone();

        if self.connections.contains_key(&name) {
            warn!(server = %name, "Server already connected, replacing");
            self.remove_server(&name)?;
        }

        let mut connection = McpConnection::new(&config)?;
        connection.start().await?;

        // Initialize the server
        self.initialize_server(&mut connection).await?;

        // Cache tools
        let tools = self.fetch_tools(&connection).await?;
        let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
        info!(server = %name, tools = ?tool_names, "MCP server initialized");

        self.tool_cache.insert(name.clone(), tools);
        self.connections.insert(name, connection);

        Ok(())
    }

    /// Initialize a server connection
    async fn initialize_server(&self, connection: &mut McpConnection) -> McpResult<()> {
        let request =
            McpRequest::new("initialize", connection.next_id()).with_params(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "clientInfo": {
                    "name": "cratos",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }));

        let response = connection.send(request).await?;

        if let Some(result) = response.result {
            let init_result: McpInitResult = serde_json::from_value(result)
                .map_err(|e| McpError::Protocol(format!("Failed to parse init result: {}", e)))?;

            debug!(
                server = %connection.name,
                protocol = %init_result.protocol_version,
                "MCP server initialized"
            );

            // Send initialized notification
            let notification = McpRequest {
                jsonrpc: "2.0".to_string(),
                method: "notifications/initialized".to_string(),
                id: connection.next_id(),
                params: None,
            };
            // Don't wait for response to notification
            let _ = connection.send(notification).await;
        }

        Ok(())
    }

    /// Fetch tools from a server
    async fn fetch_tools(&self, connection: &McpConnection) -> McpResult<Vec<McpTool>> {
        let request = McpRequest::new("tools/list", connection.next_id());
        let response = connection.send(request).await?;

        if let Some(result) = response.result {
            #[derive(Deserialize)]
            struct ToolsResult {
                tools: Vec<McpTool>,
            }

            let tools_result: ToolsResult = serde_json::from_value(result)
                .map_err(|e| McpError::Protocol(format!("Failed to parse tools: {}", e)))?;

            Ok(tools_result.tools)
        } else {
            Ok(Vec::new())
        }
    }

    /// Remove a server
    pub fn remove_server(&mut self, name: &str) -> McpResult<()> {
        if let Some(mut connection) = self.connections.remove(name) {
            connection.stop()?;
        }
        self.tool_cache.remove(name);
        Ok(())
    }

    /// List all connected servers
    pub fn list_servers(&self) -> Vec<&str> {
        self.connections.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a server is connected
    pub fn is_connected(&self, name: &str) -> bool {
        self.connections
            .get(name)
            .map(|c| c.is_active())
            .unwrap_or(false)
    }

    /// List all available tools from all servers
    pub async fn list_tools(&self) -> McpResult<Vec<(String, McpTool)>> {
        let mut all_tools = Vec::new();

        for (server_name, tools) in &self.tool_cache {
            for tool in tools {
                all_tools.push((server_name.clone(), tool.clone()));
            }
        }

        Ok(all_tools)
    }

    /// List tools from a specific server
    pub async fn list_server_tools(&self, server_name: &str) -> McpResult<Vec<McpTool>> {
        self.tool_cache
            .get(server_name)
            .cloned()
            .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))
    }

    /// Call a tool
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<McpToolResult> {
        let connection = self
            .connections
            .get(server_name)
            .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

        // Verify tool exists
        if let Some(tools) = self.tool_cache.get(server_name) {
            if !tools.iter().any(|t| t.name == tool_name) {
                return Err(McpError::ToolNotFound(
                    tool_name.to_string(),
                    server_name.to_string(),
                ));
            }
        }

        let request =
            McpRequest::new("tools/call", connection.next_id()).with_params(serde_json::json!({
                "name": tool_name,
                "arguments": arguments
            }));

        let response = connection.send(request).await?;

        if let Some(result) = response.result {
            let tool_result: McpToolResult = serde_json::from_value(result)
                .map_err(|e| McpError::Protocol(format!("Failed to parse tool result: {}", e)))?;
            Ok(tool_result)
        } else {
            Ok(McpToolResult {
                content: vec![McpContent::Text {
                    text: "No result returned".to_string(),
                }],
                is_error: true,
            })
        }
    }

    /// Call a tool by full name (server_toolname format)
    pub async fn call_tool_by_full_name(
        &self,
        full_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<McpToolResult> {
        // Parse "mcp_server_tool" format
        let parts: Vec<&str> = full_name.splitn(3, '_').collect();
        if parts.len() < 3 || parts[0] != "mcp" {
            return Err(McpError::Protocol(format!(
                "Invalid MCP tool name format: {}. Expected 'mcp_server_toolname'",
                full_name
            )));
        }

        let server_name = parts[1];
        let tool_name = parts[2];

        self.call_tool(server_name, tool_name, arguments).await
    }

    /// Refresh tool cache for all servers
    pub async fn refresh_tools(&mut self) -> McpResult<()> {
        let server_names: Vec<String> = self.connections.keys().cloned().collect();

        for name in server_names {
            if let Some(connection) = self.connections.get(&name) {
                match self.fetch_tools(connection).await {
                    Ok(tools) => {
                        self.tool_cache.insert(name.clone(), tools);
                    }
                    Err(e) => {
                        warn!(server = %name, error = %e, "Failed to refresh tools");
                    }
                }
            }
        }

        Ok(())
    }

    /// Stop all connections
    pub fn stop_all(&mut self) -> McpResult<()> {
        let names: Vec<String> = self.connections.keys().cloned().collect();
        for name in names {
            self.remove_server(&name)?;
        }
        Ok(())
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_client_config_default() {
        let config = McpClientConfig::default();
        assert!(!config.enabled);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_mcp_client_config_deserialization() {
        let json = r#"{
            "enabled": true,
            "servers": [
                {
                    "name": "filesystem",
                    "transport": {
                        "type": "stdio",
                        "command": "npx",
                        "args": ["-y", "@anthropic/mcp-server-filesystem"]
                    }
                }
            ]
        }"#;

        let config: McpClientConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].name, "filesystem");
    }

    #[test]
    fn test_parse_full_tool_name() {
        // This test verifies the parsing logic
        let full_name = "mcp_filesystem_read_file";
        let parts: Vec<&str> = full_name.splitn(3, '_').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "mcp");
        assert_eq!(parts[1], "filesystem");
        assert_eq!(parts[2], "read_file");
    }
}
