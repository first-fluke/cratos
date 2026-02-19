use crate::a2ui::protocol::A2uiServerMessage;
use crate::protocol::ServerMessage;
use crate::websocket::{BroadcastMessage, CanvasState};
use std::sync::Arc;
use uuid::Uuid;

/// Manages A2UI sessions. Wraps CanvasState to provide A2UI specific functionality.
#[derive(Clone)]
pub struct A2uiSessionManager {
    state: Arc<CanvasState>,
}

use std::fmt;

// ...

impl fmt::Debug for A2uiSessionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("A2uiSessionManager")
            .field("state", &"CanvasState") // Omit state details
            .finish()
    }
}

impl A2uiSessionManager {
    // ...
    pub fn new(state: Arc<CanvasState>) -> Self {
        Self { state }
    }

    /// Get a session wrapper.
    /// This implementation assumes the session exists in the underlying CanvasSessionManager,
    /// or loosely wraps it (sending will fail if no clients are connected).
    pub async fn get_session(&self, session_id: Uuid) -> Option<A2uiSession> {
        // Verify session existence (optional, but good for correctness)
        self.state.session_manager.get_session(session_id).await?;

        Some(A2uiSession {
            session_id,
            state: self.state.clone(),
        })
    }

    pub async fn get_or_create(&self, session_id: Uuid) -> Result<A2uiSession, String> {
        if self
            .state
            .session_manager
            .get_session(session_id)
            .await
            .is_none()
        {
            return Err(format!("Session {} not found", session_id));
        }
        Ok(A2uiSession {
            session_id,
            state: self.state.clone(),
        })
    }
}

/// Represents an active A2UI connection context for a session
pub struct A2uiSession {
    session_id: Uuid,
    state: Arc<CanvasState>,
}

impl A2uiSession {
    /// Send a message to the client (broadcast to session)
    pub async fn send(&self, msg: A2uiServerMessage) -> Result<(), String> {
        let server_msg = ServerMessage::A2ui(msg);
        let broadcast_msg = BroadcastMessage {
            session_id: self.session_id,
            origin_connection_id: None, // System/AI origin
            message: server_msg,
        };

        // Send to broadcast channel
        // Note: verify if send fails (no receivers)
        if self.state.broadcast_tx.send(broadcast_msg).is_err() {
            // It's not necessarily an error if no one is listening, but for A2UI it likely is.
            return Err("Failed to broadcast message (no active connections?)".to_string());
        }
        Ok(())
    }

    /// Wait for an event from the client
    pub async fn wait_event(
        &self,
        component_id: Uuid,
    ) -> Result<crate::a2ui::protocol::A2uiClientMessage, String> {
        let mut rx = self.state.a2ui_notify.subscribe();

        // Loop until we find the matching event
        loop {
            match rx.recv().await {
                Ok((session_id, msg)) => {
                    // Filter by session
                    if session_id != self.session_id {
                        continue;
                    }

                    // Check if it's an event for our component
                    if let crate::a2ui::protocol::A2uiClientMessage::Event {
                        component_id: event_comp_id,
                        ..
                    } = &msg
                    {
                        if *event_comp_id == component_id {
                            return Ok(msg);
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    return Err("Event channel closed".to_string());
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // If we lagged, we might have missed the event, but we continue hoping to catch a new one
                    // or we could error out. For now, continue.
                    continue;
                }
            }
        }
    }
}
