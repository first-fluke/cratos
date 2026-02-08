//! WebSocket module for Cratos
//!
//! Provides real-time communication endpoints:
//! - /ws/chat - Interactive chat WebSocket (legacy)
//! - /ws/events - Event stream WebSocket (legacy)
//! - /ws/gateway - Authenticated Gateway WebSocket (new)

pub mod chat;
pub mod events;
pub mod gateway;
pub mod protocol;

pub use chat::chat_handler;
pub use events::events_handler;
pub use gateway::gateway_handler;

use axum::{routing::get, Router};

/// Create the WebSocket router
pub fn websocket_router() -> Router {
    Router::new()
        .route("/ws/chat", get(chat_handler))
        .route("/ws/events", get(events_handler))
        .route("/ws/gateway", get(gateway_handler))
}
