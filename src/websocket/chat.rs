//! Chat WebSocket handler
//!
//! Provides real-time chat functionality via WebSocket.
//! Connected to Orchestrator for actual LLM processing
//! and EventBus for streaming execution events.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use cratos_core::event_bus::{EventBus, OrchestratorEvent};
use cratos_core::orchestrator::{Orchestrator, OrchestratorInput};
use cratos_crypto::SessionCipher;

use crate::middleware::auth::RequireAuthStrict;

/// Shared E2E cipher map (shared with sessions API via Extension)
pub type E2eCipherMap = Arc<RwLock<HashMap<Uuid, Arc<SessionCipher>>>>;

/// Chat message from client
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Send a chat message
    Chat {
        text: String,
        persona: Option<String>,
    },
    /// Send an E2E encrypted chat message
    EncryptedChat {
        /// E2E session ID (from /api/v1/sessions/init-e2e)
        e2e_session_id: Uuid,
        /// Base64-encoded nonce
        nonce: String,
        /// Base64-encoded ciphertext
        ciphertext: String,
    },
    /// Activate E2E encryption for this WS connection
    ActivateE2e {
        /// E2E session ID (from /api/v1/sessions/init-e2e)
        e2e_session_id: Uuid,
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
        active_executions: usize,
        persona: String,
    },
    /// Tool call notification
    ToolCall {
        execution_id: Uuid,
        tool_name: String,
        status: String,
    },
    /// Artifact (file, image, etc.) generated during execution
    Artifact {
        execution_id: Uuid,
        filename: String,
        mime_type: String,
        /// Base64-encoded data
        data: String,
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

/// WebSocket upgrade handler (requires strict authentication — never bypassed)
pub async fn chat_handler(
    RequireAuthStrict(_auth): RequireAuthStrict,
    ws: WebSocketUpgrade,
    Extension(orchestrator): Extension<Arc<Orchestrator>>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    e2e_ciphers: Option<Extension<E2eCipherMap>>,
) -> impl IntoResponse {
    let ciphers = e2e_ciphers.map(|Extension(c)| c);
    ws.on_upgrade(move |socket| handle_socket(socket, orchestrator, event_bus, ciphers))
}

/// Handle WebSocket connection
async fn handle_socket(
    socket: WebSocket,
    orchestrator: Arc<Orchestrator>,
    event_bus: Arc<EventBus>,
    e2e_ciphers: Option<E2eCipherMap>,
) {
    let session_id = Uuid::new_v4();
    info!("WebSocket chat connection established: {}", session_id);

    let (mut sender, mut receiver) = socket.split();

    // Send connection established message
    let connected_msg = ServerMessage::Connected { session_id };
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        let _ = sender.send(Message::Text(json)).await;
    }

    // Internal channel for sending messages from spawned tasks back to the WS sender
    let (tx, mut internal_rx) = tokio::sync::mpsc::unbounded_channel::<ServerMessage>();

    // E2E session cipher for this connection (activated by ActivateE2e message)
    let mut active_cipher: Option<Arc<SessionCipher>> = None;

    // Message handling loop: multiplex client messages and internal messages
    loop {
        tokio::select! {
            // Client messages
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        debug!("Received message: {}", text);
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(client_msg) => {
                                handle_client_message(
                                    client_msg,
                                    session_id,
                                    &orchestrator,
                                    &event_bus,
                                    &tx,
                                    &e2e_ciphers,
                                    &mut active_cipher,
                                ).await;
                            }
                            Err(e) => {
                                let error_msg = ServerMessage::Error {
                                    message: format!("Invalid message format: {}", e),
                                    code: Some("INVALID_MESSAGE".to_string()),
                                };
                                if let Ok(json) = serde_json::to_string(&error_msg) {
                                    if sender.send(Message::Text(json)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("WebSocket connection closed: {}", session_id);
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sender.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
            // Internal messages from spawned orchestrator tasks
            server_msg = internal_rx.recv() => {
                match server_msg {
                    Some(msg) => {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if sender.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                    None => break,
                }
            }
        }
    }

    info!("WebSocket chat connection ended: {}", session_id);
}

/// Handle incoming client message
async fn handle_client_message(
    msg: ClientMessage,
    session_id: Uuid,
    orchestrator: &Arc<Orchestrator>,
    event_bus: &Arc<EventBus>,
    tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>,
    e2e_ciphers: &Option<E2eCipherMap>,
    active_cipher: &mut Option<Arc<SessionCipher>>,
) {
    match msg {
        ClientMessage::ActivateE2e { e2e_session_id } => {
            handle_activate_e2e(session_id, e2e_session_id, orchestrator, tx, e2e_ciphers, active_cipher).await;
        }
        ClientMessage::EncryptedChat {
            e2e_session_id,
            nonce,
            ciphertext,
        } => {
            handle_encrypted_chat(session_id, e2e_session_id, nonce, ciphertext, orchestrator, event_bus, tx, e2e_ciphers, active_cipher).await;
        }
        ClientMessage::Chat { text, persona } => {
            handle_chat(session_id, text, persona, orchestrator, event_bus, tx).await;
        }
        ClientMessage::Status => {
            handle_status(orchestrator, tx);
        }
        ClientMessage::Cancel { execution_id } => {
            handle_cancel(execution_id, orchestrator, tx);
        }
        ClientMessage::Ping => {
            handle_ping(tx);
        }
    }
}

async fn handle_activate_e2e(
    session_id: Uuid,
    e2e_session_id: Uuid,
    orchestrator: &Arc<Orchestrator>,
    tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>,
    e2e_ciphers: &Option<E2eCipherMap>,
    active_cipher: &mut Option<Arc<SessionCipher>>,
) {
    let Some(ciphers) = e2e_ciphers else {
        let _ = tx.send(ServerMessage::Error {
            message: "E2E encryption not available".to_string(),
            code: Some("E2E_UNAVAILABLE".to_string()),
        });
        return;
    };
    let map = ciphers.read().await;
    match map.get(&e2e_session_id) {
        Some(cipher) => {
            *active_cipher = Some(cipher.clone());
            info!("E2E encryption activated for session {}", session_id);
            let _ = tx.send(ServerMessage::Status {
                connected: true,
                active_executions: orchestrator.active_execution_count().unwrap_or(0),
                persona: "cratos".to_string(),
            });
        }
        None => {
            let _ = tx.send(ServerMessage::Error {
                message: "E2E session not found. Call /api/v1/sessions/init-e2e first."
                    .to_string(),
                code: Some("E2E_SESSION_NOT_FOUND".to_string()),
            });
        }
    }
}

async fn handle_encrypted_chat(
    session_id: Uuid,
    e2e_session_id: Uuid,
    nonce: String,
    ciphertext: String,
    orchestrator: &Arc<Orchestrator>,
    event_bus: &Arc<EventBus>,
    tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>,
    e2e_ciphers: &Option<E2eCipherMap>,
    active_cipher: &mut Option<Arc<SessionCipher>>,
) {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;

    // Look up cipher — prefer active_cipher, fallback to map lookup
    let cipher = if let Some(c) = active_cipher.as_ref() {
        c.clone()
    } else if let Some(ciphers) = e2e_ciphers {
        let map = ciphers.read().await;
        match map.get(&e2e_session_id) {
            Some(c) => c.clone(),
            None => {
                let _ = tx.send(ServerMessage::Error {
                    message: "E2E session not found".to_string(),
                    code: Some("E2E_SESSION_NOT_FOUND".to_string()),
                });
                return;
            }
        }
    } else {
        let _ = tx.send(ServerMessage::Error {
            message: "E2E encryption not available".to_string(),
            code: Some("E2E_UNAVAILABLE".to_string()),
        });
        return;
    };

    // Decode nonce and ciphertext
    let nonce_bytes = match b64.decode(&nonce) {
        Ok(b) => b,
        Err(e) => {
            let _ = tx.send(ServerMessage::Error {
                message: format!("Invalid nonce base64: {}", e),
                code: Some("E2E_DECODE_ERROR".to_string()),
            });
            return;
        }
    };
    if nonce_bytes.len() != 12 {
        let _ = tx.send(ServerMessage::Error {
            message: "Nonce must be exactly 12 bytes".to_string(),
            code: Some("E2E_DECODE_ERROR".to_string()),
        });
        return;
    }
    let mut nonce_arr = [0u8; 12];
    nonce_arr.copy_from_slice(&nonce_bytes);

    let ciphertext_bytes = match b64.decode(&ciphertext) {
        Ok(b) => b,
        Err(e) => {
            let _ = tx.send(ServerMessage::Error {
                message: format!("Invalid ciphertext base64: {}", e),
                code: Some("E2E_DECODE_ERROR".to_string()),
            });
            return;
        }
    };

    let encrypted = cratos_crypto::EncryptedData {
        version: 1,
        nonce: nonce_arr,
        ciphertext: ciphertext_bytes,
    };

    // Decrypt
    let plaintext = match cipher.decrypt(&encrypted) {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(ServerMessage::Error {
                message: format!("Decryption failed: {}", e),
                code: Some("E2E_DECRYPT_ERROR".to_string()),
            });
            return;
        }
    };

    let text = match String::from_utf8(plaintext) {
        Ok(t) => t,
        Err(_) => {
            let _ = tx.send(ServerMessage::Error {
                message: "Decrypted data is not valid UTF-8".to_string(),
                code: Some("E2E_UTF8_ERROR".to_string()),
            });
            return;
        }
    };

    // Process as regular chat
    let input = OrchestratorInput::new(
        "websocket",
        session_id.to_string(),
        session_id.to_string(),
        &text,
    );

    let mut event_rx = event_bus.subscribe();
    let tx_stream = tx.clone();
    let tx_final = tx.clone();
    let orchestrator = orchestrator.clone();

    let stream_handle =
        tokio::spawn(
            async move { stream_events(&mut event_rx, &tx_stream, "cratos").await },
        );

    tokio::spawn(async move {
        match orchestrator.process(input).await {
            Ok(result) => {
                // Send final text response
                let _ = tx_final.send(ServerMessage::ChatResponse {
                    execution_id: result.execution_id,
                    text: result.response,
                    is_final: true,
                    persona: "cratos".to_string(),
                });

                // Send artifacts (files, images, etc.)
                for artifact in &result.artifacts {
                    let _ = tx_final.send(ServerMessage::Artifact {
                        execution_id: result.execution_id,
                        filename: artifact.name.clone(),
                        mime_type: artifact.mime_type.clone(),
                        data: artifact.data.clone(),
                    });
                }
            }
            Err(e) => {
                let _ = tx_final.send(ServerMessage::Error {
                    message: format!("Execution failed: {}", e),
                    code: Some("EXECUTION_ERROR".to_string()),
                });
            }
        }
        stream_handle.abort();
    });
}

async fn handle_chat(
    session_id: Uuid,
    text: String,
    persona: Option<String>,
    orchestrator: &Arc<Orchestrator>,
    event_bus: &Arc<EventBus>,
    tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>,
) {
    let active_persona = persona.unwrap_or_else(|| "cratos".to_string());

    // Build orchestrator input
    let input = OrchestratorInput::new(
        "websocket",
        session_id.to_string(),
        session_id.to_string(),
        &text,
    );

    // Subscribe to EventBus BEFORE spawning orchestrator to capture all events
    let mut event_rx = event_bus.subscribe();
    let tx_stream = tx.clone();
    let persona_for_stream = active_persona.clone();

    // Spawn the orchestrator processing
    let orchestrator = orchestrator.clone();
    let tx_final = tx.clone();
    let persona_for_final = active_persona.clone();

    // Spawn a task to forward EventBus events to the WS sender
    let stream_handle = tokio::spawn(async move {
        stream_events(&mut event_rx, &tx_stream, &persona_for_stream).await
    });

    // Spawn orchestrator.process() and send the final result
    tokio::spawn(async move {
        match orchestrator.process(input).await {
            Ok(result) => {
                // Send final text response
                let _ = tx_final.send(ServerMessage::ChatResponse {
                    execution_id: result.execution_id,
                    text: result.response,
                    is_final: true,
                    persona: persona_for_final,
                });

                // Send artifacts (files, images, etc.)
                for artifact in &result.artifacts {
                    let _ = tx_final.send(ServerMessage::Artifact {
                        execution_id: result.execution_id,
                        filename: artifact.name.clone(),
                        mime_type: artifact.mime_type.clone(),
                        data: artifact.data.clone(),
                    });
                }
            }
            Err(e) => {
                let _ = tx_final.send(ServerMessage::Error {
                    message: format!("Execution failed: {}", e),
                    code: Some("EXECUTION_ERROR".to_string()),
                });
            }
        }
        // Abort the event stream task once orchestrator is done
        stream_handle.abort();
    });
}

fn handle_status(
    orchestrator: &Arc<Orchestrator>,
    tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>,
) {
    let active = orchestrator.active_execution_count().unwrap_or(0);
    let _ = tx.send(ServerMessage::Status {
        connected: true,
        active_executions: active,
        persona: "cratos".to_string(),
    });
}

fn handle_cancel(
    execution_id: Option<Uuid>,
    orchestrator: &Arc<Orchestrator>,
    tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>,
) {
    if let Some(id) = execution_id {
        let cancelled = orchestrator.cancel_execution(id);
        debug!("Cancel execution {}: {}", id, cancelled);
    }
    let active = orchestrator.active_execution_count().unwrap_or(0);
    let _ = tx.send(ServerMessage::Status {
        connected: true,
        active_executions: active,
        persona: "cratos".to_string(),
    });
}

fn handle_ping(tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>) {
    let _ = tx.send(ServerMessage::Pong);
}


/// Stream EventBus events to the WS sender until the execution completes or the channel closes.
async fn stream_events(
    event_rx: &mut broadcast::Receiver<OrchestratorEvent>,
    tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>,
    persona: &str,
) {
    loop {
        match event_rx.recv().await {
            Ok(event) => {
                let server_msg = match &event {
                    OrchestratorEvent::ChatDelta {
                        execution_id,
                        delta,
                        is_final,
                    } => Some(ServerMessage::ChatResponse {
                        execution_id: *execution_id,
                        text: delta.clone(),
                        is_final: *is_final,
                        persona: persona.to_string(),
                    }),
                    OrchestratorEvent::ToolStarted {
                        execution_id,
                        tool_name,
                        ..
                    } => Some(ServerMessage::ToolCall {
                        execution_id: *execution_id,
                        tool_name: tool_name.clone(),
                        status: "started".to_string(),
                    }),
                    OrchestratorEvent::ToolCompleted {
                        execution_id,
                        tool_name,
                        success,
                        ..
                    } => Some(ServerMessage::ToolCall {
                        execution_id: *execution_id,
                        tool_name: tool_name.clone(),
                        status: if *success {
                            "completed".to_string()
                        } else {
                            "failed".to_string()
                        },
                    }),
                    OrchestratorEvent::ExecutionCompleted { .. }
                    | OrchestratorEvent::ExecutionFailed { .. }
                    | OrchestratorEvent::ExecutionCancelled { .. } => {
                        // Execution finished — stop streaming
                        return;
                    }
                    _ => None,
                };
                if let Some(msg) = server_msg {
                    if tx.send(msg).is_err() {
                        return;
                    }
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("Event subscriber lagged by {} events", n);
            }
            Err(broadcast::error::RecvError::Closed) => {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_deserialization() {
        let json = r#"{"type":"chat","text":"Hello","persona":null}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::Chat { text, .. } if text == "Hello"));
    }

    #[test]
    fn test_server_message_serialization() {
        let msg = ServerMessage::ChatResponse {
            execution_id: Uuid::nil(),
            text: "Hi".to_string(),
            is_final: true,
            persona: "cratos".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"chat_response\""));
        assert!(json.contains("\"is_final\":true"));
    }

    #[test]
    fn test_status_message_serialization() {
        let msg = ServerMessage::Status {
            connected: true,
            active_executions: 2,
            persona: "cratos".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"active_executions\":2"));
    }

    #[test]
    fn test_ping_deserialization() {
        let json = r#"{"type":"ping"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::Ping));
    }

    #[test]
    fn test_cancel_deserialization() {
        let id = Uuid::new_v4();
        let json = format!(r#"{{"type":"cancel","execution_id":"{}"}}"#, id);
        let msg: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(msg, ClientMessage::Cancel { execution_id } if execution_id == Some(id)));
    }

    #[test]
    fn test_artifact_message_serialization() {
        let msg = ServerMessage::Artifact {
            execution_id: Uuid::nil(),
            filename: "test.png".to_string(),
            mime_type: "image/png".to_string(),
            data: "base64data".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"artifact\""));
        assert!(json.contains("\"filename\":\"test.png\""));
        assert!(json.contains("\"mime_type\":\"image/png\""));
    }
}
