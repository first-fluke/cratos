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
pub mod connection;
mod dispatch;
pub(crate) mod events;
mod handlers;

#[cfg(test)]
mod tests;

pub use browser_relay::{BrowserRelay, SharedBrowserRelay};
pub use dispatch::dispatch_method_public;
pub use events::convert_event;

use axum::{extract::ws::WebSocketUpgrade, response::IntoResponse, Extension};
use connection::handle_gateway;
use cratos_core::{
    a2a::A2aRouter, approval::SharedApprovalManager, auth::AuthStore, event_bus::EventBus,
    nodes::NodeRegistry, Orchestrator,
};
use std::sync::Arc;

/// Maximum size of a single WS text message (1 MB).
const MAX_MESSAGE_BYTES: usize = 1_048_576;

/// WebSocket upgrade handler for `/ws/gateway`.
#[allow(clippy::too_many_arguments)]
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
            handle_gateway(
                socket,
                auth_store,
                event_bus,
                node_registry,
                a2a_router,
                browser_relay,
                orchestrator,
                approval_mgr,
            )
        })
}
