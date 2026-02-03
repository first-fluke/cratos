//! Cratos Canvas - Live Canvas System
//!
//! This crate provides the Live Canvas system for Cratos:
//! - Document: Canvas document and block types
//! - Session: Session management for editing contexts
//! - Protocol: WebSocket client/server message types
//! - WebSocket: Real-time canvas updates handler
//! - Renderer: Markdown, code, and diagram rendering
//! - Store: Persistent session storage
//! - Error: Error types for canvas operations
//! - Events: Replay event types for audit logging
//!
//! ## Features
//!
//! - Real-time collaborative document editing
//! - Multiple block types (markdown, code, diagrams, charts, images)
//! - AI-assisted content generation with streaming
//! - Syntax highlighting for code blocks
//! - Diagram rendering via Kroki
//! - Session persistence with SQLite
//! - Event recording for replay functionality
//!
//! ## Usage
//!
//! ```ignore
//! use cratos_canvas::{
//!     CanvasDocument, CanvasBlock, CanvasSessionManager,
//!     CanvasState, canvas_ws_handler,
//! };
//! use axum::{Router, routing::get};
//! use std::sync::Arc;
//!
//! // Create session manager
//! let session_manager = Arc::new(CanvasSessionManager::new());
//! let canvas_state = Arc::new(CanvasState::new(session_manager));
//!
//! // Create router with WebSocket endpoint
//! let app: Router<()> = Router::new()
//!     .route("/api/v1/canvas/ws/:session_id", get(canvas_ws_handler))
//!     .with_state(canvas_state);
//! ```
//!
//! ## Configuration
//!
//! ```toml
//! [canvas]
//! enabled = true
//! port = 8081
//! max_sessions = 100
//!
//! [canvas.websocket]
//! heartbeat_interval_secs = 30
//! max_message_size_kb = 1024
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod document;
pub mod error;
pub mod events;
pub mod protocol;
pub mod renderer;
pub mod session;
pub mod store;
pub mod websocket;

// Re-export main types
pub use document::{CanvasBlock, CanvasDocument, ChartType, DiagramType};
pub use error::{Error, Result};
pub use events::{
    AiCompletedPayload, AiErrorPayload, AiRequestStartedPayload, BlockAddedPayload,
    BlockDeletedPayload, BlockMovedPayload, BlockUpdatedPayload, CanvasEvent, CanvasEventRecorder,
    CanvasEventType, CanvasTimelineEntry,
};
pub use protocol::{ClientMessage, ConnectionState, ServerMessage, UpdateSource};
pub use renderer::{ContentRenderer, RenderedBlock};
pub use session::{CanvasSession, CanvasSessionManager};
pub use store::{SessionStore, SessionSummary};
pub use websocket::{canvas_ws_handler, BroadcastMessage, CanvasState};
