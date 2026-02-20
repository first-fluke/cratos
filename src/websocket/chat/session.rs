//! Chat WebSocket session management

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use cratos_core::event_bus::EventBus;
use cratos_core::orchestrator::Orchestrator;
use cratos_crypto::SessionCipher;

use super::handlers::{handle_client_message, E2eCipherMap};
use super::protocol::{ClientMessage, ServerMessage};

/// Handle WebSocket connection
pub async fn handle_socket(
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
    let (tx, mut internal_rx) = tokio::sync::mpsc::unbounded_channel::<Arc<ServerMessage>>();

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
                        if let Ok(json) = serde_json::to_string(&*msg) {
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
