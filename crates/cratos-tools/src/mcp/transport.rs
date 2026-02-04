//! MCP Transport Layer
//!
//! Handles communication with MCP servers over different transports.

use super::protocol::{McpError, McpRequest, McpResponse, McpResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name (unique identifier)
    pub name: String,
    /// Transport configuration
    pub transport: McpTransport,
    /// Whether to auto-start on client initialization
    #[serde(default = "default_true")]
    pub auto_start: bool,
}

fn default_true() -> bool {
    true
}

/// MCP transport type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpTransport {
    /// Stdio transport (spawn process)
    Stdio {
        /// Command to run
        command: String,
        /// Command arguments
        #[serde(default)]
        args: Vec<String>,
        /// Environment variables
        #[serde(default)]
        env: HashMap<String, String>,
    },
    /// SSE transport (HTTP)
    Sse {
        /// Server URL
        url: String,
        /// Optional API key
        #[serde(default)]
        api_key: Option<String>,
    },
}

/// Active MCP server connection
pub struct McpConnection {
    /// Server name
    pub name: String,
    /// Request ID counter
    request_id: AtomicU64,
    /// Pending requests
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<McpResponse>>>>,
    /// Child process (for stdio transport)
    process: Option<Arc<Mutex<Child>>>,
    /// Stdin writer
    stdin: Option<Arc<Mutex<std::process::ChildStdin>>>,
    /// Transport type
    transport: McpTransport,
}

impl McpConnection {
    /// Create a new connection from config
    pub fn new(config: &McpServerConfig) -> McpResult<Self> {
        Ok(Self {
            name: config.name.clone(),
            request_id: AtomicU64::new(1),
            pending: Arc::new(Mutex::new(HashMap::new())),
            process: None,
            stdin: None,
            transport: config.transport.clone(),
        })
    }

    /// Start the connection
    pub async fn start(&mut self) -> McpResult<()> {
        // Clone transport data to avoid borrow issues
        let transport = self.transport.clone();

        match transport {
            McpTransport::Stdio { command, args, env } => {
                self.start_stdio(&command, &args, &env).await
            }
            McpTransport::Sse { url, .. } => {
                warn!(url = %url, "SSE transport not yet implemented");
                Err(McpError::Transport(
                    "SSE transport not yet implemented".to_string(),
                ))
            }
        }
    }

    /// Start stdio transport
    async fn start_stdio(
        &mut self,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> McpResult<()> {
        info!(command = %command, args = ?args, "Starting MCP server process");

        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Add environment variables
        for (key, value) in env {
            // Expand environment variable references
            let expanded = if value.starts_with("${") && value.ends_with('}') {
                let var_name = &value[2..value.len() - 1];
                match std::env::var(var_name) {
                    Ok(val) => val,
                    Err(_) => {
                        warn!(
                            var = %var_name,
                            key = %key,
                            "Environment variable not found, using empty string"
                        );
                        String::new()
                    }
                }
            } else {
                value.clone()
            };
            cmd.env(key, expanded);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::Transport(format!("Failed to spawn MCP server: {}", e)))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::Transport("Failed to get stdin handle".to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::Transport("Failed to get stdout handle".to_string()))?;

        self.stdin = Some(Arc::new(Mutex::new(stdin)));
        self.process = Some(Arc::new(Mutex::new(child)));

        // Start reader thread
        let pending = self.pending.clone();
        let server_name = self.name.clone();

        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) if !line.is_empty() => {
                        debug!(server = %server_name, line = %line, "Received from MCP server");

                        match serde_json::from_str::<McpResponse>(&line) {
                            Ok(response) => {
                                // Recover from poisoned mutex to ensure responses are delivered
                                // even if another thread panicked while holding the lock
                                let mut pending = pending.lock().unwrap_or_else(|e| e.into_inner());
                                if let Some(sender) = pending.remove(&response.id) {
                                    let _ = sender.send(response);
                                }
                            }
                            Err(e) => {
                                warn!(server = %server_name, error = %e, "Failed to parse response");
                            }
                        }
                    }
                    Ok(_) => {} // Empty line
                    Err(e) => {
                        error!(server = %server_name, error = %e, "Read error");
                        break;
                    }
                }
            }
            info!(server = %server_name, "MCP server reader thread exited");
        });

        Ok(())
    }

    /// Send a request and wait for response
    pub async fn send(&self, request: McpRequest) -> McpResult<McpResponse> {
        let stdin = self
            .stdin
            .as_ref()
            .ok_or_else(|| McpError::Transport("Connection not started".to_string()))?;

        // Register pending request
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.insert(request.id, tx);
        }

        // Serialize and send
        let json = serde_json::to_string(&request)
            .map_err(|e| McpError::Protocol(format!("Failed to serialize request: {}", e)))?;

        debug!(server = %self.name, request = %json, "Sending to MCP server");

        {
            let mut stdin = stdin.lock().unwrap_or_else(|e| e.into_inner());
            writeln!(stdin, "{}", json)
                .map_err(|e| McpError::Transport(format!("Failed to write to stdin: {}", e)))?;
            stdin
                .flush()
                .map_err(|e| McpError::Transport(format!("Failed to flush stdin: {}", e)))?;
        }

        // Wait for response with timeout
        let response = tokio::time::timeout(std::time::Duration::from_secs(30), async move {
            rx.await
                .map_err(|_| McpError::Transport("Response channel closed".to_string()))
        })
        .await
        .map_err(|_| McpError::Timeout)??;

        // Check for error
        if let Some(error) = response.error {
            return Err(McpError::Server {
                code: error.code,
                message: error.message,
            });
        }

        Ok(response)
    }

    /// Get next request ID
    pub fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Stop the connection
    pub fn stop(&mut self) -> McpResult<()> {
        if let Some(process) = self.process.take() {
            let mut process = process.lock().unwrap_or_else(|e| e.into_inner());
            let _ = process.kill();
            info!(server = %self.name, "MCP server process stopped");
        }
        self.stdin = None;
        Ok(())
    }

    /// Check if connection is active
    pub fn is_active(&self) -> bool {
        self.stdin.is_some()
    }
}

impl Drop for McpConnection {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_config_deserialization() {
        let json = r#"{
            "name": "filesystem",
            "transport": {
                "type": "stdio",
                "command": "npx",
                "args": ["-y", "@anthropic/mcp-server-filesystem"],
                "env": {"HOME": "/home/user"}
            }
        }"#;

        let config: McpServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.name, "filesystem");

        match config.transport {
            McpTransport::Stdio { command, args, env } => {
                assert_eq!(command, "npx");
                assert_eq!(args.len(), 2);
                assert!(env.contains_key("HOME"));
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[test]
    fn test_sse_config_deserialization() {
        let json = r#"{
            "name": "remote",
            "transport": {
                "type": "sse",
                "url": "https://example.com/mcp",
                "api_key": "secret"
            }
        }"#;

        let config: McpServerConfig = serde_json::from_str(json).unwrap();
        match config.transport {
            McpTransport::Sse { url, api_key } => {
                assert_eq!(url, "https://example.com/mcp");
                assert_eq!(api_key, Some("secret".to_string()));
            }
            _ => panic!("Expected SSE transport"),
        }
    }
}
