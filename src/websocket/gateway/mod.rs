//! Gateway WebSocket handler.
//!
//! Authenticated WebSocket endpoint with request/response/event framing.
//!
//! Flow:
//! 1. Client sends `connect` Request with auth token
//! 2. Server validates token, binds AuthContext to connection
//! 3. Client can now invoke scoped methods (chat.send, session.list, etc.)
//! 4. Server streams OrchestratorEvents as Event frames

pub(crate) mod browser_relay;
mod dispatch;
pub(crate) mod events;
mod handlers;

pub use browser_relay::{BrowserRelay, SharedBrowserRelay};
pub use dispatch::dispatch_method_public;
pub use events::convert_event;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use cratos_core::{
    a2a::A2aRouter,
    approval::SharedApprovalManager,
    auth::{AuthContext, AuthStore},
    event_bus::EventBus,
    nodes::NodeRegistry,
    Orchestrator,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::protocol::{
    ConnectParams, ConnectResult, GatewayError, GatewayErrorCode, GatewayFrame,
};
use dispatch::DispatchContext;

/// Max time without receiving a message before considering the connection dead.
const HEARTBEAT_TIMEOUT_SECS: u64 = 60;
/// How often to send server-side pings.
const PING_INTERVAL_SECS: u64 = 30;
/// How often to send application-level keep-alive to browser extensions.
/// MV3 service workers die after 30s idle; WS protocol pings don't trigger
/// JS onmessage, so we must send text frames to keep the SW alive.
const BROWSER_KEEPALIVE_SECS: u64 = 20;
/// Maximum size of a single WS text message (1 MB).
const MAX_MESSAGE_BYTES: usize = 1_048_576;

/// WebSocket upgrade handler for `/ws/gateway`.
pub async fn gateway_handler(
    ws: WebSocketUpgrade,
    Extension(auth_store): Extension<Arc<AuthStore>>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Extension(node_registry): Extension<Arc<NodeRegistry>>,
    Extension(a2a_router): Extension<Arc<A2aRouter>>,
    Extension(browser_relay): Extension<SharedBrowserRelay>,
    Extension(orchestrator): Extension<Arc<Orchestrator>>,
    approval_manager: Option<Extension<SharedApprovalManager>>,
) -> impl IntoResponse {
    let approval_mgr = approval_manager.map(|Extension(am)| am);
    ws.max_message_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| {
            handle_gateway(socket, auth_store, event_bus, node_registry, a2a_router, browser_relay, orchestrator, approval_mgr)
        })
}

/// Main gateway connection handler.
async fn handle_gateway(
    socket: WebSocket,
    auth_store: Arc<AuthStore>,
    event_bus: Arc<EventBus>,
    node_registry: Arc<NodeRegistry>,
    a2a_router: Arc<A2aRouter>,
    browser_relay: SharedBrowserRelay,
    orchestrator: Arc<Orchestrator>,
    approval_manager: Option<SharedApprovalManager>,
) {
    let conn_id = Uuid::new_v4();
    info!(conn_id = %conn_id, "Gateway WS connection opened");

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Phase 1: Wait for `connect` handshake (with timeout)
    let (auth, role) = match wait_for_connect(&mut ws_tx, &mut ws_rx, &auth_store, conn_id).await {
        Some(pair) => pair,
        None => {
            info!(conn_id = %conn_id, "Gateway WS closed during handshake");
            return;
        }
    };

    let user_id = auth.user_id.clone();
    let is_browser = role == "browser";
    info!(conn_id = %conn_id, user = %user_id, role = %role, "Gateway authenticated");

    // If this is a browser extension, register its relay channel
    let mut relay_rx: Option<tokio::sync::mpsc::UnboundedReceiver<String>> = None;
    if is_browser {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        browser_relay
            .register(browser_relay::ExtensionConnection {
                conn_id,
                tx,
            })
            .await;
        relay_rx = Some(rx);
    }

    // Phase 2: Authenticated message loop with event forwarding
    let mut event_rx = event_bus.subscribe();
    let ping_interval =
        tokio::time::interval(tokio::time::Duration::from_secs(PING_INTERVAL_SECS));
    tokio::pin!(ping_interval);

    // Application-level keep-alive for browser extensions (MV3 SW idle workaround)
    let browser_keepalive = tokio::time::interval(tokio::time::Duration::from_secs(
        if is_browser { BROWSER_KEEPALIVE_SECS } else { 86400 }, // effectively disabled for non-browser
    ));
    tokio::pin!(browser_keepalive);

    let mut last_recv = tokio::time::Instant::now();
    let heartbeat_timeout = tokio::time::Duration::from_secs(HEARTBEAT_TIMEOUT_SECS);

    loop {
        tokio::select! {
            // Client message
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        last_recv = tokio::time::Instant::now();

                        // Browser extension may send Response frames (relay answers)
                        if is_browser {
                            if let Ok(GatewayFrame::Response { id, result, error }) = serde_json::from_str::<GatewayFrame>(&text) {
                                browser_relay.handle_response(&id, result, error).await;
                                continue;
                            }
                        }

                        if let Some(response) = handle_message(&text, &auth, &node_registry, &a2a_router, &browser_relay, &orchestrator, &event_bus, approval_manager.as_ref()).await {
                            let json = serde_json::to_string(&response).unwrap_or_default();
                            if ws_tx.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        last_recv = tokio::time::Instant::now();
                        let _ = ws_tx.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_recv = tokio::time::Instant::now();
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        warn!(conn_id = %conn_id, error = %e, "Gateway WS error");
                        break;
                    }
                    _ => {}
                }
            }
            // Relay messages → forward to browser extension
            relay_msg = async {
                match relay_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Some(json) = relay_msg {
                    if ws_tx.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
            // EventBus events → forward to client
            event = event_rx.recv() => {
                match event {
                    Ok(orchestrator_event) => {
                        if let Some(frame) = convert_event(&orchestrator_event) {
                            let json = serde_json::to_string(&frame).unwrap_or_default();
                            if ws_tx.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!(conn_id = %conn_id, lagged = n, "Event subscriber lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            // Server ping (WS protocol level)
            _ = ping_interval.tick() => {
                if last_recv.elapsed() > heartbeat_timeout {
                    info!(conn_id = %conn_id, "Heartbeat timeout, closing");
                    break;
                }
                if ws_tx.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
            // Application-level keep-alive for browser extension (text frame)
            _ = browser_keepalive.tick() => {
                if is_browser {
                    let ping_frame = r#"{"frame":"ping"}"#;
                    if ws_tx.send(Message::Text(ping_frame.to_string())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    // Cleanup browser relay
    if is_browser {
        browser_relay.unregister(conn_id).await;
    }

    info!(conn_id = %conn_id, user = %user_id, "Gateway WS connection closed");
}

/// Wait for the `connect` Request and authenticate.
/// Returns `None` if the connection should be terminated.
/// Returns `(AuthContext, role)` on success.
async fn wait_for_connect(
    ws_tx: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    ws_rx: &mut futures_util::stream::SplitStream<WebSocket>,
    auth_store: &AuthStore,
    conn_id: Uuid,
) -> Option<(AuthContext, String)> {
    let timeout = tokio::time::Duration::from_secs(10);

    let msg = tokio::time::timeout(timeout, ws_rx.next()).await;

    let text = match msg {
        Ok(Some(Ok(Message::Text(text)))) => text,
        _ => {
            // Timeout or non-text message before connect
            let frame = GatewayFrame::err(
                "",
                GatewayError::new(GatewayErrorCode::NotConnected, "Expected connect request"),
            );
            let json = serde_json::to_string(&frame).unwrap_or_default();
            let _ = ws_tx.send(Message::Text(json)).await;
            let _ = ws_tx.send(Message::Close(None)).await;
            return None;
        }
    };

    // Parse the frame
    let frame: GatewayFrame = match serde_json::from_str(&text) {
        Ok(f) => f,
        Err(_) => {
            let frame = GatewayFrame::err(
                "",
                GatewayError::new(GatewayErrorCode::InvalidParams, "Invalid frame format"),
            );
            let json = serde_json::to_string(&frame).unwrap_or_default();
            let _ = ws_tx.send(Message::Text(json)).await;
            let _ = ws_tx.send(Message::Close(None)).await;
            return None;
        }
    };

    // Must be a Request with method "connect"
    let (request_id, params_value) = match frame {
        GatewayFrame::Request {
            id, method, params, ..
        } if method == "connect" => (id, params),
        GatewayFrame::Request { id, method, .. } => {
            let frame = GatewayFrame::err(
                id,
                GatewayError::new(
                    GatewayErrorCode::NotConnected,
                    format!("Must connect first, got method: {}", method),
                ),
            );
            let json = serde_json::to_string(&frame).unwrap_or_default();
            let _ = ws_tx.send(Message::Text(json)).await;
            let _ = ws_tx.send(Message::Close(None)).await;
            return None;
        }
        _ => {
            let frame = GatewayFrame::err(
                "",
                GatewayError::new(GatewayErrorCode::NotConnected, "Expected connect Request"),
            );
            let json = serde_json::to_string(&frame).unwrap_or_default();
            let _ = ws_tx.send(Message::Text(json)).await;
            let _ = ws_tx.send(Message::Close(None)).await;
            return None;
        }
    };

    // Parse connect params
    let connect_params: ConnectParams = match serde_json::from_value(params_value) {
        Ok(p) => p,
        Err(e) => {
            let frame = GatewayFrame::err(
                request_id,
                GatewayError::new(
                    GatewayErrorCode::InvalidParams,
                    format!("Invalid connect params: {}", e),
                ),
            );
            let json = serde_json::to_string(&frame).unwrap_or_default();
            let _ = ws_tx.send(Message::Text(json)).await;
            let _ = ws_tx.send(Message::Close(None)).await;
            return None;
        }
    };

    // Validate token
    let auth = match auth_store.validate_token(&connect_params.token) {
        Ok(auth) => auth,
        Err(_) => {
            let frame = GatewayFrame::err(
                request_id,
                GatewayError::new(GatewayErrorCode::Unauthorized, "Invalid or expired token"),
            );
            let json = serde_json::to_string(&frame).unwrap_or_default();
            let _ = ws_tx.send(Message::Text(json)).await;
            let _ = ws_tx.send(Message::Close(None)).await;
            return None;
        }
    };

    // Send success response
    let scope_names: Vec<String> = auth.scopes.iter().map(|s| format!("{:?}", s)).collect();
    let session_id = Uuid::new_v4();
    let role = connect_params.role.clone();
    let result = ConnectResult {
        session_id,
        scopes: scope_names,
        protocol_version: connect_params.protocol_version.min(1),
    };
    let frame = GatewayFrame::ok(request_id, serde_json::to_value(result).unwrap());
    let json = serde_json::to_string(&frame).unwrap_or_default();
    let _ = ws_tx.send(Message::Text(json)).await;

    debug!(conn_id = %conn_id, session = %session_id, role = %role, "Gateway connect handshake complete");

    Some((auth, role))
}

/// Handle an authenticated message. Returns a response frame if applicable.
async fn handle_message(
    text: &str,
    auth: &AuthContext,
    node_registry: &NodeRegistry,
    a2a_router: &A2aRouter,
    browser_relay: &SharedBrowserRelay,
    orchestrator: &Arc<Orchestrator>,
    event_bus: &Arc<EventBus>,
    approval_manager: Option<&SharedApprovalManager>,
) -> Option<GatewayFrame> {
    let frame: GatewayFrame = match serde_json::from_str(text) {
        Ok(f) => f,
        Err(e) => {
            return Some(GatewayFrame::err(
                "",
                GatewayError::new(
                    GatewayErrorCode::InvalidParams,
                    format!("Invalid frame: {}", e),
                ),
            ));
        }
    };

    match frame {
        GatewayFrame::Request { id, method, params } => {
            let ctx = DispatchContext {
                auth,
                node_registry,
                a2a_router,
                browser_relay,
                orchestrator,
                event_bus,
                approval_manager,
            };
            Some(dispatch::dispatch_method(&id, &method, params, &ctx).await)
        }
        // Clients shouldn't send Response or Event frames
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cratos_core::a2a::A2aRouter;
    use cratos_core::auth::{AuthMethod, Scope};
    use cratos_core::event_bus::EventBus;
    use cratos_core::nodes::NodeRegistry;
    use cratos_core::{Orchestrator, OrchestratorConfig};
    use cratos_tools::ToolRegistry;

    fn admin_auth() -> AuthContext {
        AuthContext {
            user_id: "admin".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![
                Scope::Admin,
                Scope::SessionRead,
                Scope::SessionWrite,
                Scope::ExecutionRead,
                Scope::ExecutionWrite,
                Scope::ApprovalRespond,
                Scope::ConfigRead,
                Scope::ConfigWrite,
                Scope::NodeManage,
            ],
            session_id: None,
            device_id: None,
        }
    }

    fn test_node_registry() -> NodeRegistry {
        NodeRegistry::new()
    }

    fn test_a2a_router() -> A2aRouter {
        A2aRouter::new(100)
    }

    fn test_browser_relay() -> SharedBrowserRelay {
        Arc::new(BrowserRelay::new())
    }

    fn test_orchestrator() -> Arc<Orchestrator> {
        let provider: Arc<dyn cratos_llm::LlmProvider> = Arc::new(cratos_llm::MockProvider::new());
        let registry = Arc::new(ToolRegistry::new());
        Arc::new(Orchestrator::new(provider, registry, OrchestratorConfig::default()))
    }

    fn test_event_bus() -> Arc<EventBus> {
        Arc::new(EventBus::new(16))
    }

    #[tokio::test]
    async fn test_handle_message_invalid_json() {
        let nr = test_node_registry();
        let a2a = test_a2a_router();
        let br = test_browser_relay();
        let orch = test_orchestrator();
        let eb = test_event_bus();
        let result = handle_message("not json", &admin_auth(), &nr, &a2a, &br, &orch, &eb, None).await;
        assert!(result.is_some());
        match result.unwrap() {
            GatewayFrame::Response { error: Some(e), .. } => {
                assert_eq!(e.code, GatewayErrorCode::InvalidParams);
            }
            _ => panic!("expected error"),
        }
    }

    #[tokio::test]
    async fn test_handle_message_ignores_non_request() {
        let nr = test_node_registry();
        let a2a = test_a2a_router();
        let br = test_browser_relay();
        let orch = test_orchestrator();
        let eb = test_event_bus();
        // Response frame from client should be ignored
        let frame = GatewayFrame::Response {
            id: "1".to_string(),
            result: Some(serde_json::json!({})),
            error: None,
        };
        let json = serde_json::to_string(&frame).unwrap();
        let result = handle_message(&json, &admin_auth(), &nr, &a2a, &br, &orch, &eb, None).await;
        assert!(result.is_none());
    }
}
