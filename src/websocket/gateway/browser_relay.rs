//! Browser Extension Relay
//!
//! Manages connected browser extension WebSocket senders.
//! Allows the server to send requests to the extension and await responses.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, oneshot};
use tracing::debug;
use uuid::Uuid;

use crate::websocket::protocol::{GatewayError, GatewayFrame};

/// Timeout for extension request→response round-trip.
const RELAY_TIMEOUT_SECS: u64 = 30;

/// A connected browser extension.
pub struct ExtensionConnection {
    pub conn_id: Uuid,
    pub tx: mpsc::UnboundedSender<String>,
}

/// Registry of connected browser extensions + pending request tracking.
pub struct BrowserRelay {
    /// Active extension connections.
    connections: Mutex<Vec<ExtensionConnection>>,
    /// Pending requests awaiting a response from the extension.
    pending: Mutex<HashMap<String, oneshot::Sender<Result<Value, GatewayError>>>>,
}

impl BrowserRelay {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(Vec::new()),
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Check if any extension is connected.
    pub async fn is_connected(&self) -> bool {
        let conns = self.connections.lock().await;
        conns.iter().any(|c| !c.tx.is_closed())
    }

    /// Register a new browser extension connection.
    pub async fn register(&self, conn: ExtensionConnection) {
        debug!(conn_id = %conn.conn_id, "Browser extension registered");
        let mut conns = self.connections.lock().await;
        // Remove stale connections
        conns.retain(|c| !c.tx.is_closed());
        conns.push(conn);
    }

    /// Remove a connection by ID.
    pub async fn unregister(&self, conn_id: Uuid) {
        let mut conns = self.connections.lock().await;
        conns.retain(|c| c.conn_id != conn_id);
        debug!(conn_id = %conn_id, "Browser extension unregistered");
    }

    /// Send a request to the first available extension and await response.
    pub async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = format!("relay-{}", Uuid::new_v4());

        // Build request frame
        let frame = GatewayFrame::Request {
            id: id.clone(),
            method: method.to_string(),
            params,
        };
        let json = serde_json::to_string(&frame).map_err(|e| e.to_string())?;

        // Create oneshot channel for the response
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id.clone(), tx);
        }

        // Send to first available connection
        let sent = {
            let conns = self.connections.lock().await;
            let mut sent = false;
            for conn in conns.iter() {
                if conn.tx.send(json.clone()).is_ok() {
                    sent = true;
                    break;
                }
            }
            sent
        };

        if !sent {
            let mut pending = self.pending.lock().await;
            pending.remove(&id);
            return Err("No browser extension connected".to_string());
        }

        // Await response with timeout
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(RELAY_TIMEOUT_SECS),
            rx,
        )
        .await
        {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(gw_err))) => Err(gw_err.message),
            Ok(Err(_)) => {
                // Sender dropped (connection closed)
                let mut pending = self.pending.lock().await;
                pending.remove(&id);
                Err("Extension connection closed".to_string())
            }
            Err(_) => {
                // Timeout
                let mut pending = self.pending.lock().await;
                pending.remove(&id);
                Err("Extension request timed out".to_string())
            }
        }
    }

    /// Handle a response frame from the extension, matching it to a pending request.
    pub async fn handle_response(
        &self,
        id: &str,
        result: Option<Value>,
        error: Option<GatewayError>,
    ) {
        let mut pending = self.pending.lock().await;
        if let Some(tx) = pending.remove(id) {
            let response = if let Some(err) = error {
                Err(err)
            } else {
                Ok(result.unwrap_or(Value::Null))
            };
            let _ = tx.send(response);
        } else {
            debug!(id = %id, "No pending request for relay response");
        }
    }
}

impl Default for BrowserRelay {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: wrap `BrowserRelay` in `Arc` for shared use.
pub type SharedBrowserRelay = Arc<BrowserRelay>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_relay_no_connections() {
        let relay = BrowserRelay::new();
        assert!(!relay.is_connected().await);

        let result = relay.send_request("browser.get_tabs", Value::Null).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No browser extension"));
    }

    #[tokio::test]
    async fn test_relay_register_unregister() {
        let relay = BrowserRelay::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let conn_id = Uuid::new_v4();

        relay.register(ExtensionConnection { conn_id, tx }).await;
        assert!(relay.is_connected().await);

        relay.unregister(conn_id).await;
        assert!(!relay.is_connected().await);
    }
}
