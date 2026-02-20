//! Chat WebSocket event handlers

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

use cratos_core::event_bus::{EventBus, OrchestratorEvent};
use cratos_core::orchestrator::{Orchestrator, OrchestratorInput};
use cratos_crypto::SessionCipher;

use super::protocol::{ClientMessage, ServerMessage};

/// Shared E2E cipher map
pub type E2eCipherMap = Arc<RwLock<HashMap<Uuid, Arc<SessionCipher>>>>;

/// Handle incoming client message
pub async fn handle_client_message(
    msg: ClientMessage,
    session_id: Uuid,
    orchestrator: &Arc<Orchestrator>,
    event_bus: &Arc<EventBus>,
    tx: &mpsc::UnboundedSender<Arc<ServerMessage>>,
    e2e_ciphers: &Option<E2eCipherMap>,
    active_cipher: &mut Option<Arc<SessionCipher>>,
) {
    match msg {
        ClientMessage::ActivateE2e { e2e_session_id } => {
            handle_activate_e2e(
                session_id,
                e2e_session_id,
                orchestrator,
                tx,
                e2e_ciphers,
                active_cipher,
            )
            .await;
        }
        ClientMessage::EncryptedChat {
            e2e_session_id,
            nonce,
            ciphertext,
        } => {
            handle_encrypted_chat(
                session_id,
                e2e_session_id,
                nonce,
                ciphertext,
                orchestrator,
                event_bus,
                tx,
                e2e_ciphers,
                active_cipher,
            )
            .await;
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
    tx: &mpsc::UnboundedSender<Arc<ServerMessage>>,
    e2e_ciphers: &Option<E2eCipherMap>,
    active_cipher: &mut Option<Arc<SessionCipher>>,
) {
    let Some(ciphers) = e2e_ciphers else {
        let _ = tx.send(Arc::new(ServerMessage::Error {
            message: "E2E encryption not available".to_string(),
            code: Some("E2E_UNAVAILABLE".to_string()),
        }));
        return;
    };
    let map = ciphers.read().await;
    match map.get(&e2e_session_id) {
        Some(cipher) => {
            *active_cipher = Some(cipher.clone());
            info!("E2E encryption activated for session {}", session_id);
            let _ = tx.send(Arc::new(ServerMessage::Status {
                connected: true,
                active_executions: orchestrator.active_execution_count().unwrap_or(0),
                persona: "cratos".to_string(),
            }));
        }
        None => {
            let _ = tx.send(Arc::new(ServerMessage::Error {
                message: "E2E session not found. Call /api/v1/sessions/init-e2e first.".to_string(),
                code: Some("E2E_SESSION_NOT_FOUND".to_string()),
            }));
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_encrypted_chat(
    session_id: Uuid,
    e2e_session_id: Uuid,
    nonce: String,
    ciphertext: String,
    orchestrator: &Arc<Orchestrator>,
    event_bus: &Arc<EventBus>,
    tx: &mpsc::UnboundedSender<Arc<ServerMessage>>,
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
                let _ = tx.send(Arc::new(ServerMessage::Error {
                    message: "E2E session not found".to_string(),
                    code: Some("E2E_SESSION_NOT_FOUND".to_string()),
                }));
                return;
            }
        }
    } else {
        let _ = tx.send(Arc::new(ServerMessage::Error {
            message: "E2E encryption not available".to_string(),
            code: Some("E2E_UNAVAILABLE".to_string()),
        }));
        return;
    };

    // Decode nonce and ciphertext
    let nonce_bytes = match b64.decode(&nonce) {
        Ok(b) => b,
        Err(e) => {
            let _ = tx.send(Arc::new(ServerMessage::Error {
                message: format!("Invalid nonce base64: {}", e),
                code: Some("E2E_DECODE_ERROR".to_string()),
            }));
            return;
        }
    };
    if nonce_bytes.len() != 12 {
        let _ = tx.send(Arc::new(ServerMessage::Error {
            message: "Nonce must be exactly 12 bytes".to_string(),
            code: Some("E2E_DECODE_ERROR".to_string()),
        }));
        return;
    }
    let mut nonce_arr = [0u8; 12];
    nonce_arr.copy_from_slice(&nonce_bytes);

    let ciphertext_bytes = match b64.decode(&ciphertext) {
        Ok(b) => b,
        Err(e) => {
            let _ = tx.send(Arc::new(ServerMessage::Error {
                message: format!("Invalid ciphertext base64: {}", e),
                code: Some("E2E_DECODE_ERROR".to_string()),
            }));
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
            let _ = tx.send(Arc::new(ServerMessage::Error {
                message: format!("Decryption failed: {}", e),
                code: Some("E2E_DECRYPT_ERROR".to_string()),
            }));
            return;
        }
    };

    let text = match String::from_utf8(plaintext) {
        Ok(t) => t,
        Err(_) => {
            let _ = tx.send(Arc::new(ServerMessage::Error {
                message: "Decrypted data is not valid UTF-8".to_string(),
                code: Some("E2E_UTF8_ERROR".to_string()),
            }));
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

    run_orchestrator(
        orchestrator.clone(),
        event_bus.clone(),
        input,
        tx.clone(),
        "cratos".to_string(),
    )
    .await;
}

async fn handle_chat(
    session_id: Uuid,
    text: String,
    persona: Option<String>,
    orchestrator: &Arc<Orchestrator>,
    event_bus: &Arc<EventBus>,
    tx: &mpsc::UnboundedSender<Arc<ServerMessage>>,
) {
    let active_persona = persona.unwrap_or_else(|| "cratos".to_string());

    // Build orchestrator input
    let input = OrchestratorInput::new(
        "websocket",
        session_id.to_string(),
        session_id.to_string(),
        &text,
    );

    run_orchestrator(
        orchestrator.clone(),
        event_bus.clone(),
        input,
        tx.clone(),
        active_persona,
    )
    .await;
}

fn handle_status(orchestrator: &Arc<Orchestrator>, tx: &mpsc::UnboundedSender<Arc<ServerMessage>>) {
    let active = orchestrator.active_execution_count().unwrap_or(0);
    let _ = tx.send(Arc::new(ServerMessage::Status {
        connected: true,
        active_executions: active,
        persona: "cratos".to_string(),
    }));
}

fn handle_cancel(
    execution_id: Option<Uuid>,
    orchestrator: &Arc<Orchestrator>,
    tx: &mpsc::UnboundedSender<Arc<ServerMessage>>,
) {
    if let Some(id) = execution_id {
        let cancelled = orchestrator.cancel_execution(id);
        debug!("Cancel execution {}: {}", id, cancelled);
    }
    let active = orchestrator.active_execution_count().unwrap_or(0);
    let _ = tx.send(Arc::new(ServerMessage::Status {
        connected: true,
        active_executions: active,
        persona: "cratos".to_string(),
    }));
}

fn handle_ping(tx: &mpsc::UnboundedSender<Arc<ServerMessage>>) {
    let _ = tx.send(Arc::new(ServerMessage::Pong));
}

/// Run orchestrator processing and stream events
async fn run_orchestrator(
    orchestrator: Arc<Orchestrator>,
    event_bus: Arc<EventBus>,
    input: OrchestratorInput,
    tx: mpsc::UnboundedSender<Arc<ServerMessage>>,
    persona: String,
) {
    // Subscribe to EventBus BEFORE spawning orchestrator to capture all events
    let event_rx = event_bus.subscribe();
    let tx_stream = tx.clone();
    let persona_for_stream = persona.clone();

    // Spawn a task to forward EventBus events to the WS sender
    let stream_handle =
        tokio::spawn(async move { stream_events(event_rx, &tx_stream, &persona_for_stream).await });

    // Spawn orchestrator.process() and send the final result
    let tx_final = tx.clone();
    let persona_for_final = persona.clone();

    tokio::spawn(async move {
        match orchestrator.process(input).await {
            Ok(result) => {
                // Send final text response
                let _ = tx_final.send(Arc::new(ServerMessage::ChatResponse {
                    execution_id: result.execution_id,
                    text: result.response,
                    is_final: true,
                    persona: persona_for_final,
                }));

                // Send artifacts (files, images, etc.)
                for artifact in &result.artifacts {
                    let _ = tx_final.send(Arc::new(ServerMessage::Artifact {
                        execution_id: result.execution_id,
                        filename: artifact.name.clone(),
                        mime_type: artifact.mime_type.clone(),
                        data: artifact.data.clone(),
                    }));
                }
            }
            Err(e) => {
                let _ = tx_final.send(Arc::new(ServerMessage::Error {
                    message: format!("Execution failed: {}", e),
                    code: Some("EXECUTION_ERROR".to_string()),
                }));
            }
        }
        // Abort the event stream task once orchestrator is done
        stream_handle.abort();
    });
}

/// Stream EventBus events to the WS sender
async fn stream_events(
    mut event_rx: broadcast::Receiver<OrchestratorEvent>,
    tx: &mpsc::UnboundedSender<Arc<ServerMessage>>,
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
                    } => Some(Arc::new(ServerMessage::ChatResponse {
                        execution_id: *execution_id,
                        text: delta.clone(),
                        is_final: *is_final,
                        persona: persona.to_string(),
                    })),
                    OrchestratorEvent::ToolStarted {
                        execution_id,
                        tool_name,
                        ..
                    } => Some(Arc::new(ServerMessage::ToolCall {
                        execution_id: *execution_id,
                        tool_name: tool_name.clone(),
                        status: "started".to_string(),
                    })),
                    OrchestratorEvent::ToolCompleted {
                        execution_id,
                        tool_name,
                        success,
                        ..
                    } => Some(Arc::new(ServerMessage::ToolCall {
                        execution_id: *execution_id,
                        tool_name: tool_name.clone(),
                        status: if *success {
                            "completed".to_string()
                        } else {
                            "failed".to_string()
                        },
                    })),
                    OrchestratorEvent::ExecutionCompleted { .. }
                    | OrchestratorEvent::ExecutionFailed { .. }
                    | OrchestratorEvent::ExecutionCancelled { .. } => {
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
