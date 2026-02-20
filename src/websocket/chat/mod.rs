//! Chat WebSocket module

pub mod handlers;
pub mod protocol;
pub mod session;

use axum::{extract::ws::WebSocketUpgrade, response::IntoResponse, Extension};
use std::sync::Arc;

use crate::middleware::auth::RequireAuthStrict;
use cratos_core::event_bus::EventBus;
use cratos_core::orchestrator::Orchestrator;
pub use handlers::E2eCipherMap;

/// WebSocket upgrade handler
pub async fn chat_handler(
    RequireAuthStrict(_auth): RequireAuthStrict,
    ws: WebSocketUpgrade,
    Extension(orchestrator): Extension<Arc<Orchestrator>>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    e2e_ciphers: Option<Extension<E2eCipherMap>>,
) -> impl IntoResponse {
    let ciphers = e2e_ciphers.map(|Extension(c)| c);
    ws.on_upgrade(move |socket| session::handle_socket(socket, orchestrator, event_bus, ciphers))
}

#[cfg(test)]
mod tests;
