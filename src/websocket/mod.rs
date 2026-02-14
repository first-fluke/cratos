//! WebSocket module for Cratos
//!
//! Provides real-time communication endpoints:
//! - /ws/chat - Interactive chat WebSocket (legacy)
//! - /ws/events - Event stream WebSocket (legacy)
//! - /ws/gateway - Authenticated Gateway WebSocket (new)
//! - /ws/canvas/:session_id - Canvas document editing

pub mod canvas;
pub mod chat;
pub mod events;
pub mod gateway;
pub mod protocol;

pub use canvas::canvas_handler;
pub use chat::{chat_handler, chat_handler_public};
pub use events::events_handler;
pub use gateway::gateway_handler;

use axum::{routing::get, Router};

/// Create the WebSocket router
pub fn websocket_router() -> Router {
    Router::new()
        // Public chat endpoint for Web UI (same-origin)
        .route("/ws/chat", get(chat_handler_public))
        // Authenticated chat endpoint for external clients
        .route("/ws/chat-auth", get(chat_handler))
        .route("/ws/events", get(events_handler))
        .route("/ws/gateway", get(gateway_handler))
        .route("/ws/canvas/{session_id}", get(canvas_handler))
}
