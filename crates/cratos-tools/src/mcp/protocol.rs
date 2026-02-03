//! MCP Protocol Types
//!
//! JSON-RPC 2.0 based protocol types for MCP communication.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// MCP error type
#[derive(Debug, Error)]
pub enum McpError {
    /// Transport error (I/O, connection, etc.)
    #[error("Transport error: {0}")]
    Transport(String),

    /// Protocol error (invalid JSON-RPC, etc.)
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Server returned an error
    #[error("Server error {code}: {message}")]
    Server {
        /// Error code
        code: i32,
        /// Error message
        message: String,
    },

    /// Timeout waiting for response
    #[error("Request timed out")]
    Timeout,

    /// Server not found
    #[error("Server '{0}' not found")]
    ServerNotFound(String),

    /// Tool not found
    #[error("Tool '{0}' not found on server '{1}'")]
    ToolNotFound(String, String),
}

/// MCP Result type
pub type McpResult<T> = std::result::Result<T, McpError>;

/// JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    /// JSON-RPC version
    pub jsonrpc: String,
    /// Request method
    pub method: String,
    /// Request ID
    pub id: u64,
    /// Request parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl McpRequest {
    /// Create a new request
    pub fn new(method: impl Into<String>, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            id,
            params: None,
        }
    }

    /// Add parameters
    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }
}

/// JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    /// JSON-RPC version
    pub jsonrpc: String,
    /// Response ID (matches request ID)
    pub id: u64,
    /// Result (on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error (on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpRpcError>,
}

/// JSON-RPC error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// MCP tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name
    pub name: String,
    /// Tool description
    #[serde(default)]
    pub description: String,
    /// Input schema (JSON Schema)
    #[serde(default = "default_schema", rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

fn default_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {}
    })
}

impl McpTool {
    /// Convert to cratos-tools ToolDefinition
    pub fn to_tool_definition(&self, server_name: &str) -> crate::ToolDefinition {
        crate::ToolDefinition {
            name: format!("mcp_{}_{}", server_name, self.name),
            description: self.description.clone(),
            category: crate::ToolCategory::External,
            risk_level: crate::RiskLevel::Medium,
            parameters: self.input_schema.clone(),
            enabled: true,
            required_capabilities: vec![format!("mcp:{}", server_name)],
        }
    }
}

/// MCP tool call request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    /// Tool name
    pub name: String,
    /// Tool arguments
    #[serde(default)]
    pub arguments: HashMap<String, serde_json::Value>,
}

/// MCP tool call result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    /// Content items
    #[serde(default)]
    pub content: Vec<McpContent>,
    /// Whether the tool call resulted in an error
    #[serde(default, rename = "isError")]
    pub is_error: bool,
}

/// MCP content item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContent {
    /// Text content
    #[serde(rename = "text")]
    Text {
        /// Text content
        text: String,
    },
    /// Image content
    #[serde(rename = "image")]
    Image {
        /// Base64 encoded image data
        data: String,
        /// MIME type
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    /// Resource content
    #[serde(rename = "resource")]
    Resource {
        /// Resource URI
        uri: String,
        /// Resource text
        #[serde(default)]
        text: Option<String>,
        /// Resource blob (base64)
        #[serde(default)]
        blob: Option<String>,
    },
}

impl McpContent {
    /// Get text representation of content
    pub fn as_text(&self) -> Option<&str> {
        match self {
            McpContent::Text { text } => Some(text),
            McpContent::Resource { text: Some(t), .. } => Some(t),
            _ => None,
        }
    }
}

/// MCP server capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpCapabilities {
    /// Tool capabilities
    #[serde(default)]
    pub tools: Option<ToolCapabilities>,
    /// Resource capabilities
    #[serde(default)]
    pub resources: Option<ResourceCapabilities>,
    /// Prompt capabilities
    #[serde(default)]
    pub prompts: Option<PromptCapabilities>,
}

/// Tool capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolCapabilities {
    /// Whether tools list can change
    #[serde(default, rename = "listChanged")]
    pub list_changed: bool,
}

/// Resource capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceCapabilities {
    /// Whether resource subscriptions are supported
    #[serde(default)]
    pub subscribe: bool,
    /// Whether resource list can change
    #[serde(default, rename = "listChanged")]
    pub list_changed: bool,
}

/// Prompt capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptCapabilities {
    /// Whether prompt list can change
    #[serde(default, rename = "listChanged")]
    pub list_changed: bool,
}

/// MCP initialization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInitResult {
    /// Protocol version
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    /// Server capabilities
    #[serde(default)]
    pub capabilities: McpCapabilities,
    /// Server info
    #[serde(default, rename = "serverInfo")]
    pub server_info: Option<ServerInfo>,
}

/// Server information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Server name
    pub name: String,
    /// Server version
    #[serde(default)]
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_request_serialization() {
        let request = McpRequest::new("tools/list", 1);
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"tools/list\""));
    }

    #[test]
    fn test_mcp_tool_deserialization() {
        let json = r#"{
            "name": "read_file",
            "description": "Read a file from disk",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }
        }"#;

        let tool: McpTool = serde_json::from_str(json).unwrap();
        assert_eq!(tool.name, "read_file");
        assert!(!tool.description.is_empty());
    }

    #[test]
    fn test_mcp_content_text() {
        let content = McpContent::Text {
            text: "Hello, world!".to_string(),
        };
        assert_eq!(content.as_text(), Some("Hello, world!"));
    }
}
