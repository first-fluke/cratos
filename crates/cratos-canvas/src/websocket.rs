//! WebSocket Handler
//!
//! This module provides the WebSocket handler for real-time canvas updates.

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::document::{CanvasBlock, CanvasDocument};
use crate::protocol::{ClientMessage, ServerMessage, UpdateSource};
use crate::session::CanvasSessionManager;
use cratos_llm::{CompletionRequest, LlmRouter, Message as LlmMessage};
use tokio_util::sync::CancellationToken;

/// Shared state for the WebSocket handler
pub struct CanvasState {
    /// Session manager
    pub session_manager: Arc<CanvasSessionManager>,
    /// Broadcast channel for session updates
    pub broadcast_tx: broadcast::Sender<BroadcastMessage>,
    /// LLM router for AI features
    pub llm_router: Option<Arc<LlmRouter>>,
    /// Cancellation token for active AI requests
    pub ai_cancel: CancellationToken,
    /// Channel for forwarding A2UI messages to external handlers (e.g. Orchestrator)
    pub a2ui_tx: Option<tokio::sync::mpsc::Sender<(Uuid, crate::a2ui::A2uiClientMessage)>>,
    /// Internal broadcast for A2UI events (for tool waiting)
    pub a2ui_notify: broadcast::Sender<(Uuid, crate::a2ui::A2uiClientMessage)>,
}

impl CanvasState {
    /// Create a new canvas state
    #[must_use]
    pub fn new(session_manager: Arc<CanvasSessionManager>) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1024);
        let (a2ui_notify, _) = broadcast::channel(1024);
        Self {
            session_manager,
            broadcast_tx,
            llm_router: None,
            ai_cancel: CancellationToken::new(),
            a2ui_tx: None,
            a2ui_notify,
        }
    }

    /// Set the A2UI message channel
    pub fn with_a2ui_tx(mut self, tx: tokio::sync::mpsc::Sender<(Uuid, crate::a2ui::A2uiClientMessage)>) -> Self {
        self.a2ui_tx = Some(tx);
        self
    }

    /// Create with LLM router for AI features
    #[must_use]
    pub fn with_llm(mut self, router: Arc<LlmRouter>) -> Self {
        self.llm_router = Some(router);
        self
    }
}

/// Message broadcast to all connections in a session
#[derive(Debug, Clone)]
pub struct BroadcastMessage {
    /// Session ID
    pub session_id: Uuid,
    /// Connection ID that originated the message (to exclude from broadcast)
    pub origin_connection_id: Option<Uuid>,
    /// Server message to broadcast
    pub message: ServerMessage,
}

/// WebSocket upgrade handler
pub async fn canvas_ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<CanvasState>>,
) -> impl IntoResponse {
    info!(session_id = %session_id, "WebSocket upgrade requested");
    ws.on_upgrade(move |socket| handle_socket(socket, session_id, state))
}

/// Handle a WebSocket connection
async fn handle_socket(socket: WebSocket, session_id: Uuid, state: Arc<CanvasState>) {
    let connection_id = Uuid::new_v4();
    info!(
        session_id = %session_id,
        connection_id = %connection_id,
        "WebSocket connected"
    );

    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast channel
    let mut broadcast_rx = state.broadcast_tx.subscribe();

    // Get session and send welcome message
    let session = state.session_manager.get_session(session_id).await;
    let welcome_msg = match session {
        Some(s) => ServerMessage::welcome(session_id, s.document),
        None => {
            // Create a new session with empty document
            let doc = CanvasDocument::new("Untitled");
            let new_session = state
                .session_manager
                .create_session("anonymous", doc.clone())
                .await;
            ServerMessage::welcome(new_session.id, doc)
        }
    };

    if let Err(e) = send_message(&mut sender, &welcome_msg).await {
        error!(error = %e, "Failed to send welcome message");
        return;
    }

    // Spawn broadcast receiver task
    let sender_clone = Arc::new(tokio::sync::Mutex::new(sender));
    let sender_for_broadcast = sender_clone.clone();
    let broadcast_handle = tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            // Only forward messages for this session, excluding self-originated messages
            if msg.session_id == session_id && msg.origin_connection_id != Some(connection_id) {
                let mut sender = sender_for_broadcast.lock().await;
                if let Ok(json) = serde_json::to_string(&msg.message) {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Main message loop
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!(text = %text, "Received message");
                if let Err(e) =
                    handle_client_message(&text, session_id, connection_id, &state, &sender_clone)
                        .await
                {
                    warn!(error = %e, "Error handling message");
                    let mut sender = sender_clone.lock().await;
                    let _ =
                        send_message(&mut sender, &ServerMessage::error("error", e.to_string()))
                            .await;
                }
            }
            Ok(Message::Close(_)) => {
                info!(connection_id = %connection_id, "WebSocket closed by client");
                break;
            }
            Ok(Message::Ping(data)) => {
                let mut sender = sender_clone.lock().await;
                let _ = sender.send(Message::Pong(data)).await;
            }
            Err(e) => {
                warn!(error = %e, "WebSocket error");
                break;
            }
            _ => {}
        }
    }

    broadcast_handle.abort();
    info!(connection_id = %connection_id, "WebSocket disconnected");
}

/// Send a server message
async fn send_message(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    message: &ServerMessage,
) -> Result<(), String> {
    let json = serde_json::to_string(message).map_err(|e| e.to_string())?;
    sender
        .send(Message::Text(json))
        .await
        .map_err(|e| e.to_string())
}

/// Handle a client message
pub(crate) async fn handle_client_message(
    text: &str,
    session_id: Uuid,
    connection_id: Uuid,
    state: &Arc<CanvasState>,
    sender: &Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
) -> Result<(), String> {
    let client_msg: ClientMessage =
        serde_json::from_str(text).map_err(|e| format!("Invalid message: {}", e))?;

    match client_msg {
        ClientMessage::Ping => {
            let mut sender = sender.lock().await;
            send_message(&mut sender, &ServerMessage::Pong).await?;
        }

        ClientMessage::UpdateBlock { block_id, content } => {
            // Update the block in the session
            let updated = state
                .session_manager
                .update_session(session_id, |session| {
                    session.document.update_block(block_id, content.clone())
                })
                .await;

            if updated == Some(true) {
                // Broadcast the update to other connections
                let _ = state.broadcast_tx.send(BroadcastMessage {
                    session_id,
                    origin_connection_id: Some(connection_id),
                    message: ServerMessage::block_updated(block_id, &content, UpdateSource::User),
                });
            } else {
                let mut sender = sender.lock().await;
                send_message(
                    &mut sender,
                    &ServerMessage::error("not_found", "Block not found"),
                )
                .await?;
            }
        }

        ClientMessage::AddBlock { block, after_id } => {
            let index = state
                .session_manager
                .update_session(session_id, |session| {
                    let index = if let Some(after) = after_id {
                        session
                            .document
                            .blocks
                            .iter()
                            .position(|b| b.id() == after)
                            .map(|i| i + 1)
                            .unwrap_or(session.document.blocks.len())
                    } else {
                        session.document.blocks.len()
                    };
                    session.document.insert_block(index, block.clone());
                    index
                })
                .await;

            if let Some(idx) = index {
                let _ = state.broadcast_tx.send(BroadcastMessage {
                    session_id,
                    origin_connection_id: Some(connection_id),
                    message: ServerMessage::BlockAdded {
                        block: block.clone(),
                        index: idx,
                    },
                });
            }
        }

        ClientMessage::DeleteBlock { block_id } => {
            let removed = state
                .session_manager
                .update_session(session_id, |session| {
                    session.document.remove_block(block_id).is_some()
                })
                .await;

            if removed == Some(true) {
                let _ = state.broadcast_tx.send(BroadcastMessage {
                    session_id,
                    origin_connection_id: Some(connection_id),
                    message: ServerMessage::BlockDeleted { block_id },
                });
            }
        }

        ClientMessage::MoveBlock {
            block_id,
            new_index,
        } => {
            let moved = state
                .session_manager
                .update_session(session_id, |session| {
                    if let Some(pos) = session
                        .document
                        .blocks
                        .iter()
                        .position(|b| b.id() == block_id)
                    {
                        let block = session.document.blocks.remove(pos);
                        let idx = new_index.min(session.document.blocks.len());
                        session.document.blocks.insert(idx, block);
                        Some(idx)
                    } else {
                        None
                    }
                })
                .await
                .flatten();

            if let Some(idx) = moved {
                let _ = state.broadcast_tx.send(BroadcastMessage {
                    session_id,
                    origin_connection_id: Some(connection_id),
                    message: ServerMessage::BlockMoved {
                        block_id,
                        new_index: idx,
                    },
                });
            }
        }

        ClientMessage::AskAi {
            prompt,
            context_blocks,
            target_block_id,
        } => {
            // Create a new block for AI response if needed
            let target_id = match target_block_id {
                Some(id) => id,
                None => {
                    let block = CanvasBlock::markdown("");
                    let id = block.id();

                    // Add the block to the session
                    state
                        .session_manager
                        .update_session(session_id, |session| {
                            session.document.add_block(block.clone());
                        })
                        .await;

                    // Broadcast the new block
                    let _ = state.broadcast_tx.send(BroadcastMessage {
                        session_id,
                        origin_connection_id: None,
                        message: ServerMessage::BlockAdded {
                            block,
                            index: usize::MAX, // End of document
                        },
                    });

                    id
                }
            };

            // Notify AI started
            {
                let mut sender_guard = sender.lock().await;
                let _ = send_message(
                    &mut sender_guard,
                    &ServerMessage::AiStarted {
                        block_id: target_id,
                    },
                )
                .await;
            }

            // Build context from referenced blocks
            let context = collect_context_text(state, session_id, &context_blocks).await;
            let ai_response =
                run_ai_completion(state, &prompt, &context, target_id, session_id).await;

            // Update the block content
            state
                .session_manager
                .update_session(session_id, |session| {
                    session
                        .document
                        .update_block(target_id, ai_response.clone())
                })
                .await;

            // Send completion
            {
                let mut sender_guard = sender.lock().await;
                let _ = send_message(
                    &mut sender_guard,
                    &ServerMessage::AiCompleted {
                        block_id: target_id,
                        tokens_used: None,
                    },
                )
                .await;
            }
        }

        ClientMessage::StopAi => {
            // Cancel any active AI request for this session
            state.ai_cancel.cancel();
            debug!(session_id = %session_id, "AI cancellation requested");
        }

        ClientMessage::ExecuteCode { block_id } => {
            let code = state
                .session_manager
                .update_session(session_id, |session| {
                    session
                        .document
                        .blocks
                        .iter()
                        .find(|b| b.id() == block_id)
                        .map(|b| b.content().to_string())
                })
                .await
                .flatten();

            let output = match code {
                Some(source) => execute_sandboxed_code(&source).await,
                None => "Error: block not found".to_string(),
            };

            let mut sender_guard = sender.lock().await;
            let _ = send_message(
                &mut sender_guard,
                &ServerMessage::ExecutionOutput {
                    block_id,
                    output,
                    is_error: false,
                },
            )
            .await;
            let _ = send_message(
                &mut sender_guard,
                &ServerMessage::ExecutionCompleted {
                    block_id,
                    exit_code: 0,
                },
            )
            .await;
        }

        ClientMessage::Join { .. } | ClientMessage::Leave => {
            // Already handled at connection level
        }

        ClientMessage::A2ui(msg) => {
            // Internal broadcast for tool waiting
            let _ = state.a2ui_notify.send((session_id, msg.clone()));

            // Forward A2UI messages to external handler if configured (e.g. Orchestrator Steering)
            if let Some(tx) = &state.a2ui_tx {
                if let Err(e) = tx.send((session_id, msg.clone())).await {
                    warn!(error = %e, "Failed to forward A2UI message");
                    return Err(format!("Internal error: {}", e));
                }
            } else {
                debug!("Received A2UI message but no handler configured");
            }
        }
    }

    Ok(())
}

/// Collect text from context blocks for AI prompt
async fn collect_context_text(state: &CanvasState, session_id: Uuid, block_ids: &[Uuid]) -> String {
    if block_ids.is_empty() {
        return String::new();
    }
    let ids = block_ids.to_vec();
    state
        .session_manager
        .update_session(session_id, move |session| {
            ids.iter()
                .filter_map(|id| {
                    session
                        .document
                        .blocks
                        .iter()
                        .find(|b| b.id() == *id)
                        .map(|b| b.content().to_string())
                })
                .collect::<Vec<_>>()
                .join("\n\n---\n\n")
        })
        .await
        .unwrap_or_default()
}

/// Run AI completion via LLM router (falls back to placeholder if no router)
async fn run_ai_completion(
    state: &CanvasState,
    prompt: &str,
    context: &str,
    target_id: Uuid,
    session_id: Uuid,
) -> String {
    let Some(router) = &state.llm_router else {
        let placeholder = format!("AI response to: {}", prompt);
        stream_ai_text(state, &placeholder, target_id, session_id).await;
        return placeholder;
    };

    let mut messages = Vec::new();
    if !context.is_empty() {
        messages.push(LlmMessage::system(format!(
            "Context from document:\n\n{}",
            context
        )));
    }
    messages.push(LlmMessage::user(prompt));

    let request = CompletionRequest {
        model: String::new(), // Use provider default
        messages,
        max_tokens: Some(4096),
        temperature: Some(0.7),
        stop: None,
    };

    let cancel = state.ai_cancel.clone();
    let result = tokio::select! {
        res = router.complete(request) => res,
        _ = cancel.cancelled() => {
            return "AI request cancelled".to_string();
        }
    };

    match result {
        Ok(response) => {
            stream_ai_text(state, &response.content, target_id, session_id).await;
            response.content
        }
        Err(e) => {
            let err_msg = format!("AI error: {}", e);
            stream_ai_text(state, &err_msg, target_id, session_id).await;
            err_msg
        }
    }
}

/// Stream AI text in chunks via broadcast
async fn stream_ai_text(state: &CanvasState, text: &str, target_id: Uuid, session_id: Uuid) {
    for chunk in text.chars().collect::<Vec<_>>().chunks(20) {
        let chunk_str: String = chunk.iter().collect();
        let _ = state.broadcast_tx.send(BroadcastMessage {
            session_id,
            origin_connection_id: None,
            message: ServerMessage::ai_streaming(target_id, &chunk_str, false),
        });
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    }
}

/// Execute code in a sandboxed process
async fn execute_sandboxed_code(source: &str) -> String {
    // Simple sandboxed execution via subprocess with timeout
    let output = tokio::time::timeout(
        tokio::time::Duration::from_secs(30),
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(source)
            .output(),
    )
    .await;

    match output {
        Ok(Ok(o)) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            if stderr.is_empty() {
                stdout.to_string()
            } else {
                format!("{}\n[stderr]: {}", stdout, stderr)
            }
        }
        Ok(Err(e)) => format!("Execution error: {}", e),
        Err(_) => "Execution timed out (30s limit)".to_string(),
    }
}

#[cfg(test)]
#[path = "websocket_tests.rs"]
mod ws_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_state_creation() {
        let manager = Arc::new(CanvasSessionManager::default());
        let state = CanvasState::new(manager);
        assert!(state.broadcast_tx.receiver_count() == 0);
    }

    #[test]
    fn test_broadcast_message() {
        let msg = BroadcastMessage {
            session_id: Uuid::new_v4(),
            origin_connection_id: Some(Uuid::new_v4()),
            message: ServerMessage::Pong,
        };
        assert!(msg.origin_connection_id.is_some());
    }
}
