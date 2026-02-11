//! MCP (Model Context Protocol) JSON-RPC 2.0 compatibility layer.
//!
//! Wraps Cratos tools as MCP-compatible methods so that IDEs (VS Code, Zed)
//! can connect via `cratos acp --mcp` using the standard MCP protocol.
//!
//! Supported methods:
//! - `initialize` → server capabilities
//! - `tools/list` → tool definitions
//! - `tools/call` → execute a tool
//! - `prompts/list` → empty (reserved)
//! - `resources/list` → empty (reserved)

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info};

use cratos_core::orchestrator::Orchestrator;
use cratos_tools::ToolRegistry;

/// MCP JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// MCP JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// MCP JSON-RPC error.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

impl JsonRpcResponse {
    fn ok(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn err(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

/// MCP server capabilities.
#[derive(Debug, Serialize)]
struct ServerCapabilities {
    tools: ToolCapability,
}

#[derive(Debug, Serialize)]
struct ToolCapability {
    #[serde(rename = "listChanged")]
    list_changed: bool,
}

/// MCP tool definition (subset of JSON Schema).
#[derive(Debug, Serialize)]
struct McpToolDef {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

/// MCP-compatible bridge over stdin/stdout.
pub struct McpBridge {
    tool_registry: Arc<ToolRegistry>,
    orchestrator: Arc<Orchestrator>,
}

impl McpBridge {
    pub fn new(tool_registry: Arc<ToolRegistry>, orchestrator: Arc<Orchestrator>) -> Self {
        Self {
            tool_registry,
            orchestrator,
        }
    }

    /// Run the MCP JSON-RPC loop over stdin/stdout.
    pub async fn run(&self) -> anyhow::Result<()> {
        info!("MCP compatibility bridge started (JSON-RPC 2.0 over stdio)");

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                break; // EOF
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                Ok(request) => {
                    debug!(method = %request.method, "MCP request");
                    self.handle_request(request).await
                }
                Err(e) => JsonRpcResponse::err(None, -32700, format!("Parse error: {}", e)),
            };

            let json = serde_json::to_string(&response)?;
            stdout.write_all(json.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }

        info!("MCP bridge shutting down");
        Ok(())
    }

    async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        match req.method.as_str() {
            "initialize" => self.handle_initialize(req.id),
            "tools/list" => self.handle_tools_list(req.id),
            "tools/call" => self.handle_tools_call(req.id, req.params).await,
            "prompts/list" => JsonRpcResponse::ok(req.id, serde_json::json!({ "prompts": [] })),
            "resources/list" => {
                JsonRpcResponse::ok(req.id, serde_json::json!({ "resources": [] }))
            }
            "notifications/initialized" => {
                // Client acknowledges initialization — no response needed for notifications,
                // but since we're line-based, return ok
                JsonRpcResponse::ok(req.id, serde_json::json!({}))
            }
            _ => JsonRpcResponse::err(req.id, -32601, format!("Method not found: {}", req.method)),
        }
    }

    fn handle_initialize(&self, id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse::ok(
            id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": ServerCapabilities {
                    tools: ToolCapability { list_changed: false },
                },
                "serverInfo": {
                    "name": "cratos",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }),
        )
    }

    fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let tools: Vec<McpToolDef> = self
            .tool_registry
            .list_definitions()
            .iter()
            .map(|def| McpToolDef {
                name: def.name.clone(),
                description: Some(def.description.clone()),
                input_schema: def.parameters.clone(),
            })
            .collect();

        JsonRpcResponse::ok(id, serde_json::json!({ "tools": tools }))
    }

    async fn handle_tools_call(&self, id: Option<Value>, params: Value) -> JsonRpcResponse {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => return JsonRpcResponse::err(id, -32602, "Missing 'name' parameter"),
        };

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        match self.orchestrator.runner().execute(&name, arguments).await {
            Ok(exec_result) => {
                let content = if exec_result.result.success {
                    serde_json::json!([{
                        "type": "text",
                        "text": serde_json::to_string(&exec_result.result.output).unwrap_or_default()
                    }])
                } else {
                    serde_json::json!([{
                        "type": "text",
                        "text": exec_result.result.error.unwrap_or_else(|| "Unknown error".to_string())
                    }])
                };

                JsonRpcResponse::ok(
                    id,
                    serde_json::json!({
                        "content": content,
                        "isError": !exec_result.result.success,
                    }),
                )
            }
            Err(e) => {
                error!(tool = %name, error = %e, "MCP tool call failed");
                JsonRpcResponse::ok(
                    id,
                    serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Tool execution error: {}", e)
                        }],
                        "isError": true,
                    }),
                )
            }
        }
    }
}

/// Run the MCP-compatible bridge as a standalone process.
pub async fn run_mcp() -> anyhow::Result<()> {
    // Minimal setup for standalone MCP mode
    let provider: Arc<dyn cratos_llm::LlmProvider> = Arc::new(cratos_llm::MockProvider::new());
    let mut registry = ToolRegistry::new();
    cratos_tools::register_builtins(&mut registry);
    let registry = Arc::new(registry);

    let orchestrator = Arc::new(Orchestrator::new(
        provider,
        registry.clone(),
        Default::default(),
    ));

    let bridge = McpBridge::new(registry, orchestrator);
    bridge.run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonrpc_request_parsing() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(serde_json::json!(1)));
    }

    #[test]
    fn test_jsonrpc_response_ok() {
        let resp = JsonRpcResponse::ok(Some(serde_json::json!(1)), serde_json::json!({"ok": true}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_jsonrpc_response_err() {
        let resp = JsonRpcResponse::err(Some(serde_json::json!(1)), -32601, "Not found");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("-32601"));
    }

    #[test]
    fn test_tools_list_format() {
        let tool = McpToolDef {
            name: "exec".to_string(),
            description: Some("Execute a command".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"}
                }
            }),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("\"inputSchema\""));
        assert!(json.contains("\"exec\""));
    }

    #[tokio::test]
    async fn test_handle_initialize() {
        let provider: Arc<dyn cratos_llm::LlmProvider> =
            Arc::new(cratos_llm::MockProvider::new());
        let registry = Arc::new(ToolRegistry::new());
        let orchestrator = Arc::new(Orchestrator::new(
            provider,
            registry.clone(),
            Default::default(),
        ));
        let bridge = McpBridge::new(registry, orchestrator);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "initialize".to_string(),
            params: serde_json::json!({}),
        };
        let resp = bridge.handle_request(req).await;
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let provider: Arc<dyn cratos_llm::LlmProvider> =
            Arc::new(cratos_llm::MockProvider::new());
        let registry = Arc::new(ToolRegistry::new());
        let orchestrator = Arc::new(Orchestrator::new(
            provider,
            registry.clone(),
            Default::default(),
        ));
        let bridge = McpBridge::new(registry, orchestrator);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "nonexistent".to_string(),
            params: serde_json::json!({}),
        };
        let resp = bridge.handle_request(req).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }
}
