use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use uuid::Uuid;

use cratos_core::{
    a2a::A2aRouter,
    approval::SharedApprovalManager,
    auth::{AuthContext, AuthStore},
    event_bus::EventBus,
    nodes::NodeRegistry,
    Orchestrator,
};

use super::super::protocol::{
    ConnectParams, ConnectResult, GatewayError, GatewayErrorCode, GatewayFrame,
};
use super::browser_relay;
use super::dispatch::{dispatch_method, DispatchContext};
use super::events::convert_event;
use super::SharedBrowserRelay;

/// Max time without receiving a message before considering the connection dead.
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 60;
/// How often to send server-side pings.
pub const PING_INTERVAL_SECS: u64 = 30;
/// How often to send application-level keep-alive to browser extensions.
pub const BROWSER_KEEPALIVE_SECS: u64 = 20;

/// Main gateway connection handler.
#[allow(clippy::too_many_arguments)]
pub async fn handle_gateway(
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
            .register(browser_relay::ExtensionConnection { conn_id, tx })
            .await;
        relay_rx = Some(rx);
    }

    // Phase 2: Authenticated message loop with event forwarding
    let mut event_rx = event_bus.subscribe();
    let ping_interval = tokio::time::interval(tokio::time::Duration::from_secs(PING_INTERVAL_SECS));
    tokio::pin!(ping_interval);

    // Application-level keep-alive for browser extensions (MV3 SW idle workaround)
    let browser_keepalive = tokio::time::interval(tokio::time::Duration::from_secs(
        if is_browser {
            BROWSER_KEEPALIVE_SECS
        } else {
            86400
        }, // effectively disabled for non-browser
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
#[allow(clippy::too_many_arguments)]
pub async fn handle_message(
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
            Some(dispatch_method(&id, &method, params, &ctx).await)
        }
        _ => None,
    }
}
