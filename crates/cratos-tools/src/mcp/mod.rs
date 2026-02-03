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

mod client;
mod protocol;
mod transport;

pub use client::{McpClient, McpClientConfig};
pub use protocol::{
    McpContent, McpError, McpRequest, McpResponse, McpResult, McpTool, McpToolCall,
};
pub use transport::{McpServerConfig, McpTransport};
