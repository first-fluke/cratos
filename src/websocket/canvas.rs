//! Canvas WebSocket handler
//!
//! Wraps the cratos-canvas WebSocket handler for integration with Axum.
//! Provides real-time collaborative document editing.

use axum::{
    extract::{Extension, Path, WebSocketUpgrade},
    response::IntoResponse,
};
use cratos_canvas::CanvasState;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

/// Canvas WebSocket upgrade handler
///
/// Accepts WebSocket connections for canvas sessions.
/// If session_id is "new", creates a new session with a random UUID.
///
/// # Path Parameters
/// - `session_id`: Existing session UUID or "new" to create
///
/// # Returns
/// WebSocket upgrade response
pub async fn canvas_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    Extension(state): Extension<Arc<CanvasState>>,
) -> impl IntoResponse {
    let session_uuid = if session_id == "new" {
        let id = Uuid::new_v4();
        info!(session_id = %id, "Creating new canvas session");
        id
    } else {
        match session_id.parse::<Uuid>() {
            Ok(id) => {
                debug!(session_id = %id, "Joining existing canvas session");
                id
            }
            Err(_) => {
                let id = Uuid::new_v4();
                info!(session_id = %id, original = %session_id, "Invalid session ID, creating new");
                id
            }
        }
    };

    // Delegate to cratos-canvas handler
    cratos_canvas::canvas_ws_handler(
        ws,
        Path(session_uuid),
        axum::extract::State(state),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_parsing() {
        let valid_uuid = "550e8400-e29b-41d4-a716-446655440000";
        assert!(valid_uuid.parse::<Uuid>().is_ok());

        let invalid = "not-a-uuid";
        assert!(invalid.parse::<Uuid>().is_err());
    }
}
