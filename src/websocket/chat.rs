//! Chat WebSocket handler
//!
//! Provides real-time chat functionality via WebSocket

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use uuid::Uuid;

/// Chat message from client
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Send a chat message
    Chat {
        text: String,
        persona: Option<String>,
    },
    /// Request status
    Status,
    /// Cancel current execution
    Cancel { execution_id: Option<Uuid> },
    /// Ping for keepalive
    Ping,
}

/// Chat message to client
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ServerMessage {
    /// Chat response (may be streaming)
    ChatResponse {
        execution_id: Uuid,
        text: String,
        is_final: bool,
        persona: String,
    },
    /// Status update
    Status {
        connected: bool,
        active_executions: i32,
        persona: String,
    },
    /// Tool call notification
    ToolCall {
        execution_id: Uuid,
        tool_name: String,
        status: String,
    },
    /// Error message
    Error {
        message: String,
        code: Option<String>,
    },
    /// Pong response
    Pong,
    /// Connection established
    Connected { session_id: Uuid },
}

/// WebSocket upgrade handler
pub async fn chat_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket) {
    let session_id = Uuid::new_v4();
    info!("WebSocket chat connection established: {}", session_id);

    let (mut sender, mut receiver) = socket.split();

    // Send connection established message
    let connected_msg = ServerMessage::Connected { session_id };
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        let _ = sender.send(Message::Text(json)).await;
    }

    // Message handling loop
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Received message: {}", text);

                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        let response = handle_client_message(client_msg, session_id).await;
                        if let Ok(json) = serde_json::to_string(&response) {
                            if sender.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = ServerMessage::Error {
                            message: format!("Invalid message format: {}", e),
                            code: Some("INVALID_MESSAGE".to_string()),
                        };
                        if let Ok(json) = serde_json::to_string(&error_msg) {
                            let _ = sender.send(Message::Text(json)).await;
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket connection closed: {}", session_id);
                break;
            }
            Ok(Message::Ping(data)) => {
                let _ = sender.send(Message::Pong(data)).await;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    info!("WebSocket chat connection ended: {}", session_id);
}

/// Handle incoming client message
async fn handle_client_message(msg: ClientMessage, _session_id: Uuid) -> ServerMessage {
    match msg {
        ClientMessage::Chat { text, persona } => {
            // In production, this would send to the Orchestrator
            // For now, return a mock response
            let execution_id = Uuid::new_v4();
            let active_persona = persona.unwrap_or_else(|| "cratos".to_string());

            // Simulate processing
            debug!("Processing chat: {} (persona: {})", text, active_persona);

            ServerMessage::ChatResponse {
                execution_id,
                text: format!("[{}] I received your message: \"{}\"", active_persona, text),
                is_final: true,
                persona: active_persona,
            }
        }
        ClientMessage::Status => ServerMessage::Status {
            connected: true,
            active_executions: 0,
            persona: "cratos".to_string(),
        },
        ClientMessage::Cancel { execution_id } => {
            if let Some(id) = execution_id {
                debug!("Cancelling execution: {}", id);
            }
            ServerMessage::Status {
                connected: true,
                active_executions: 0,
                persona: "cratos".to_string(),
            }
        }
        ClientMessage::Ping => ServerMessage::Pong,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_chat_message() {
        let session_id = Uuid::new_v4();
        let msg = ClientMessage::Chat {
            text: "Hello".to_string(),
            persona: None,
        };

        let response = handle_client_message(msg, session_id).await;
        if let ServerMessage::ChatResponse { text, is_final, .. } = response {
            assert!(text.contains("Hello"));
            assert!(is_final);
        } else {
            panic!("Expected ChatResponse");
        }
    }

    #[tokio::test]
    async fn test_handle_status_message() {
        let session_id = Uuid::new_v4();
        let msg = ClientMessage::Status;

        let response = handle_client_message(msg, session_id).await;
        if let ServerMessage::Status { connected, .. } = response {
            assert!(connected);
        } else {
            panic!("Expected Status");
        }
    }

    #[tokio::test]
    async fn test_handle_ping_message() {
        let session_id = Uuid::new_v4();
        let msg = ClientMessage::Ping;

        let response = handle_client_message(msg, session_id).await;
        assert!(matches!(response, ServerMessage::Pong));
    }
}
