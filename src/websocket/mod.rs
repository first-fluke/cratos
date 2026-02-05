//! WebSocket module for Cratos
//!
//! Provides real-time communication endpoints:
//! - /ws/chat - Interactive chat WebSocket
//! - /ws/events - Event stream WebSocket

pub mod chat;
pub mod events;

pub use chat::chat_handler;
pub use events::events_handler;

use axum::{routing::get, Router};

/// Create the WebSocket router
pub fn websocket_router() -> Router {
    Router::new()
        .route("/ws/chat", get(chat_handler))
        .route("/ws/events", get(events_handler))
}
