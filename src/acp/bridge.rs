//! ACP Bridge — stdin/stdout JSON-lines ↔ internal Gateway dispatch.
//!
//! Reads JSON-line requests from stdin, dispatches them through the same
//! logic as the Gateway WS handler, and writes responses/events to stdout.
//!
//! Localhost connections can optionally skip authentication.

use cratos_core::{
    a2a::A2aRouter,
    auth::{AuthContext, AuthMethod, AuthStore, Scope},
    event_bus::EventBus,
    nodes::NodeRegistry,
    Orchestrator,
};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, info, warn};

use super::protocol::AcpMessage;
use crate::websocket::gateway::browser_relay::{BrowserRelay, SharedBrowserRelay};
use crate::websocket::protocol::{GatewayError, GatewayErrorCode};

/// ACP bridge state.
pub struct AcpBridge {
    auth_store: Arc<AuthStore>,
    event_bus: Arc<EventBus>,
    node_registry: Arc<NodeRegistry>,
    a2a_router: Arc<A2aRouter>,
    browser_relay: SharedBrowserRelay,
    orchestrator: Arc<Orchestrator>,
}

impl AcpBridge {
    /// Create a new ACP bridge.
    pub fn new(
        auth_store: Arc<AuthStore>,
        event_bus: Arc<EventBus>,
        node_registry: Arc<NodeRegistry>,
        a2a_router: Arc<A2aRouter>,
        orchestrator: Arc<Orchestrator>,
    ) -> Self {
        Self {
            auth_store,
            event_bus,
            node_registry,
            a2a_router,
            browser_relay: Arc::new(BrowserRelay::new()),
            orchestrator,
        }
    }

    /// Run the bridge, reading from stdin and writing to stdout.
    ///
    /// If `token` is provided, it's used for authentication.
    /// If `None` and the auth store has auth disabled, localhost trust is used.
    pub async fn run(&self, token: Option<String>) -> anyhow::Result<()> {
        info!("ACP bridge starting (stdin/stdout JSON-lines)");

        // Authenticate
        let auth = self.authenticate(token.as_deref())?;
        info!(user = %auth.user_id, "ACP bridge authenticated");

        // Send ready event
        let ready = AcpMessage::Event {
            event: "bridge.ready".to_string(),
            data: serde_json::json!({
                "user_id": auth.user_id,
                "scopes": auth.scopes.iter().map(|s| format!("{:?}", s)).collect::<Vec<_>>(),
            }),
        };
        self.write_message(&ready).await?;

        // Subscribe to EventBus for forwarding events
        let mut event_rx = self.event_bus.subscribe();

        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        loop {
            tokio::select! {
                // Read from stdin
                line = lines.next_line() => {
                    match line {
                        Ok(Some(line)) => {
                            if line.trim().is_empty() {
                                continue;
                            }
                            let response = self.handle_line(&line, &auth).await;
                            self.write_message(&response).await?;
                        }
                        Ok(None) => {
                            // EOF — stdin closed
                            debug!("ACP bridge: stdin closed");
                            break;
                        }
                        Err(e) => {
                            warn!(error = %e, "ACP bridge: stdin read error");
                            break;
                        }
                    }
                }
                // Forward EventBus events to stdout
                event = event_rx.recv() => {
                    match event {
                        Ok(orch_event) => {
                            if let Some(frame) = crate::websocket::gateway::convert_event(&orch_event) {
                                let msg: AcpMessage = frame.into();
                                self.write_message(&msg).await?;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            debug!(lagged = n, "ACP event subscriber lagged");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }

        info!("ACP bridge stopped");
        Ok(())
    }

    /// Authenticate the bridge session.
    fn authenticate(&self, token: Option<&str>) -> anyhow::Result<AuthContext> {
        if let Some(token) = token {
            // Use provided token
            self.auth_store
                .validate_token(token)
                .map_err(|e| anyhow::anyhow!("Authentication failed: {}", e))
        } else if !self.auth_store.is_enabled() {
            // Localhost trust: auth disabled → anonymous admin
            Ok(AuthContext {
                user_id: "acp-local".to_string(),
                method: AuthMethod::ApiKey,
                scopes: vec![Scope::Admin],
                session_id: None,
                device_id: None,
            })
        } else {
            Err(anyhow::anyhow!(
                "Authentication required. Provide --token or disable auth for local development."
            ))
        }
    }

    /// Handle a single JSON-line from stdin.
    async fn handle_line(&self, line: &str, auth: &AuthContext) -> AcpMessage {
        // Parse as AcpMessage
        let msg: AcpMessage = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => {
                return AcpMessage::Response {
                    id: String::new(),
                    result: None,
                    error: Some(GatewayError::new(
                        GatewayErrorCode::InvalidParams,
                        format!("Invalid JSON: {}", e),
                    )),
                };
            }
        };

        match msg {
            AcpMessage::Request { id, method, params } => {
                // Reuse the Gateway dispatch logic
                let frame = crate::websocket::gateway::dispatch_method_public(
                    &id,
                    &method,
                    params,
                    auth,
                    &self.node_registry,
                    &self.a2a_router,
                    &self.browser_relay,
                    &self.orchestrator,
                    &self.event_bus,
                    None,
                )
                .await;
                frame.into()
            }
            // Client shouldn't send Response or Event
            _ => AcpMessage::Response {
                id: String::new(),
                result: None,
                error: Some(GatewayError::new(
                    GatewayErrorCode::InvalidParams,
                    "Only Request messages are accepted",
                )),
            },
        }
    }

    /// Write a message as a JSON line to stdout.
    async fn write_message(&self, msg: &AcpMessage) -> anyhow::Result<()> {
        let json = serde_json::to_string(msg)?;
        let mut stdout = tokio::io::stdout();
        stdout.write_all(json.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
        Ok(())
    }
}

/// Standalone entry point for `cratos acp` CLI command.
///
/// Creates minimal infrastructure (AuthStore, EventBus, NodeRegistry)
/// and runs the bridge.
pub async fn run_acp(token: Option<String>) -> anyhow::Result<()> {
    // Minimal setup: we need auth store, event bus, node registry
    // In a full implementation, these would be shared with a running server.
    // For standalone ACP, we create lightweight instances.
    let auth_store = Arc::new(AuthStore::new(false)); // localhost trust by default
    let event_bus = Arc::new(EventBus::new(256));
    let node_registry = Arc::new(NodeRegistry::new());
    let a2a_router = Arc::new(A2aRouter::default());
    let provider: Arc<dyn cratos_llm::LlmProvider> = Arc::new(cratos_llm::MockProvider::new());
    let registry = Arc::new(cratos_tools::ToolRegistry::new());
    let orchestrator = Arc::new(Orchestrator::new(
        provider,
        registry,
        cratos_core::OrchestratorConfig::default(),
    ));

    let bridge = AcpBridge::new(
        auth_store,
        event_bus,
        node_registry,
        a2a_router,
        orchestrator,
    );
    bridge.run(token).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_orchestrator() -> Arc<Orchestrator> {
        let provider: Arc<dyn cratos_llm::LlmProvider> = Arc::new(cratos_llm::MockProvider::new());
        let registry = Arc::new(cratos_tools::ToolRegistry::new());
        Arc::new(Orchestrator::new(
            provider,
            registry,
            cratos_core::OrchestratorConfig::default(),
        ))
    }

    #[test]
    fn test_authenticate_disabled_auth() {
        let auth_store = Arc::new(AuthStore::new(false));
        let event_bus = Arc::new(EventBus::new(16));
        let node_registry = Arc::new(NodeRegistry::new());
        let a2a_router = Arc::new(A2aRouter::default());
        let bridge = AcpBridge::new(
            auth_store,
            event_bus,
            node_registry,
            a2a_router,
            test_orchestrator(),
        );

        let auth = bridge.authenticate(None).unwrap();
        assert_eq!(auth.user_id, "acp-local");
        assert!(auth.has_scope(&Scope::Admin));
    }

    #[test]
    fn test_authenticate_with_token() {
        let auth_store = Arc::new(AuthStore::new(true));
        let (key, _) = auth_store
            .generate_api_key("test", vec![Scope::SessionRead], "test")
            .unwrap();
        let event_bus = Arc::new(EventBus::new(16));
        let node_registry = Arc::new(NodeRegistry::new());
        let a2a_router = Arc::new(A2aRouter::default());
        let bridge = AcpBridge::new(
            auth_store,
            event_bus,
            node_registry,
            a2a_router,
            test_orchestrator(),
        );

        let auth = bridge.authenticate(Some(key.expose())).unwrap();
        assert_eq!(auth.user_id, "test");
    }

    #[test]
    fn test_authenticate_required_but_no_token() {
        let auth_store = Arc::new(AuthStore::new(true));
        let event_bus = Arc::new(EventBus::new(16));
        let node_registry = Arc::new(NodeRegistry::new());
        let a2a_router = Arc::new(A2aRouter::default());
        let bridge = AcpBridge::new(
            auth_store,
            event_bus,
            node_registry,
            a2a_router,
            test_orchestrator(),
        );

        let result = bridge.authenticate(None);
        assert!(result.is_err());
    }
}
