//! MCP (Model Context Protocol) Client
//!
//! This module provides MCP client functionality for connecting to external MCP servers.
//! MCP servers can extend Cratos with additional tools without modifying the core codebase.
//!
//! ## Supported Transports
//!
//! - **stdio**: Spawns a child process and communicates via stdin/stdout (JSON-RPC)
//! - **sse**: Server-Sent Events over HTTP (planned)
//!
//! ## Usage
//!
//! ```no_run
//! use cratos_tools::mcp::{McpClient, McpServerConfig, McpTransport};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = McpServerConfig {
//!     name: "filesystem".to_string(),
//!     transport: McpTransport::Stdio {
//!         command: "npx".to_string(),
//!         args: vec!["-y".to_string(), "@anthropic/mcp-server-filesystem".to_string()],
//!         env: Default::default(),
//!     },
//!     auto_start: true,
//! };
//!
//! let mut client = McpClient::new();
//! client.add_server(config).await?;
//!
//! let tools = client.list_tools().await?;
//! # Ok(())
//! # }
//! ```

mod bridge;
mod client;
mod protocol;
mod transport;

pub use bridge::McpToolBridge;
pub use client::{McpClient, McpClientConfig};
pub use protocol::{
    McpContent, McpError, McpRequest, McpResponse, McpResult, McpTool, McpToolCall,
};
pub use transport::{McpServerConfig, McpTransport};

use crate::registry::ToolRegistry;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Entry in `.mcp.json` `mcpServers` map.
#[derive(Debug, serde::Deserialize)]
struct McpJsonEntry {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    #[allow(dead_code)]
    description: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Top-level `.mcp.json` schema.
#[derive(Debug, serde::Deserialize)]
struct McpJsonFile {
    #[serde(rename = "mcpServers")]
    mcp_servers: HashMap<String, McpJsonEntry>,
}

/// Load `.mcp.json`, start enabled servers, and register their tools
/// into the given [`ToolRegistry`].
///
/// Returns a shared `McpClient` for later use (e.g. the orchestrator
/// can hold a reference for dynamic tool calls).
pub async fn register_mcp_tools(
    registry: &mut ToolRegistry,
    mcp_json_path: &Path,
) -> crate::Result<Arc<RwLock<McpClient>>> {
    let content = std::fs::read_to_string(mcp_json_path)
        .map_err(|e| crate::Error::Config(format!("Failed to read .mcp.json: {}", e)))?;

    let mcp_file: McpJsonFile = serde_json::from_str(&content)
        .map_err(|e| crate::Error::Config(format!("Failed to parse .mcp.json: {}", e)))?;

    let mut client = McpClient::new();
    let mut registered_tool_count = 0usize;

    for (name, entry) in &mcp_file.mcp_servers {
        if !entry.enabled {
            info!(server = %name, "MCP server disabled, skipping");
            continue;
        }

        let server_config = McpServerConfig {
            name: name.clone(),
            transport: McpTransport::Stdio {
                command: entry.command.clone(),
                args: entry.args.clone(),
                env: entry.env.clone(),
            },
            auto_start: true,
        };

        // Give each server 10s to start + initialize; skip if too slow
        match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client.add_server(server_config),
        )
        .await
        {
            Ok(Ok(())) => info!(server = %name, "MCP server started"),
            Ok(Err(e)) => {
                warn!(server = %name, error = %e, "Failed to start MCP server, skipping");
                continue;
            }
            Err(_) => {
                warn!(server = %name, "MCP server start timed out (10s), skipping");
                continue;
            }
        }
    }

    let client = Arc::new(RwLock::new(client));

    // Collect tools from all connected servers and register them
    let tools = {
        let c = client.read().await;
        c.list_tools().await.unwrap_or_default()
    };

    for (server_name, mcp_tool) in tools {
        let definition = mcp_tool.to_tool_definition(&server_name);
        let tool_name = definition.name.clone();

        let bridge = McpToolBridge::new(
            definition,
            server_name,
            mcp_tool.name.clone(),
            client.clone(),
        );

        registry.register(Arc::new(bridge));
        registered_tool_count += 1;
        info!(tool = %tool_name, "Registered MCP tool");
    }

    info!(count = registered_tool_count, "MCP tool registration complete");
    Ok(client)
}
