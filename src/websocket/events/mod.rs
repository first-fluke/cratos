//! Events WebSocket handler
//!
//! Provides real-time event stream functionality.

use std::sync::Arc;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tracing::{debug, error, info};
use uuid::Uuid;

use cratos_core::event_bus::{EventBus, OrchestratorEvent};
use crate::middleware::auth::RequireAuthStrict;

mod types;


pub use types::{EventNotification, SubscriptionRequest, SubscriptionState};

/// WebSocket upgrade handler
pub async fn events_handler(
    RequireAuthStrict(_auth): RequireAuthStrict,
    ws: WebSocketUpgrade,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, event_bus))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, event_bus: Arc<EventBus>) {
    let session_id = Uuid::new_v4();
    info!("WebSocket events connection established: {}", session_id);

    let (mut sender, mut receiver) = socket.split();

    // Send connection established message
    let connected_msg = EventNotification::Connected { session_id };
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        let _ = sender.send(Message::Text(json)).await;
    }

    // Track subscriptions
    let mut state = SubscriptionState::new();

    // Subscribe to EventBus
    let mut event_rx = event_bus.subscribe();

    // Message handling loop
    loop {
        tokio::select! {
            // Client messages
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        debug!("Received subscription request: {}", text);
                        match serde_json::from_str::<SubscriptionRequest>(&text) {
                            Ok(request) => {
                                let response = handle_subscription_request(request, &mut state);
                                if let Ok(json) = serde_json::to_string(&response) {
                                    if sender.send(Message::Text(json)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                let error_msg = EventNotification::Error {
                                    message: format!("Invalid message format: {}", e),
                                    code: Some("INVALID_MESSAGE".to_string()),
                                };
                                if let Ok(json) = serde_json::to_string(&error_msg) {
                                    let _ = sender.send(Message::Text(json)).await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("WebSocket events connection closed: {}", session_id);
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
            // EventBus events
            event = event_rx.recv() => {
                match event {
                    Ok(orchestrator_event) => {
                        if let Some(notification) = convert_event(&orchestrator_event, &state) {
                            if let Ok(json) = serde_json::to_string(&notification) {
                                if sender.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!(session_id = %session_id, lagged = n, "Event subscriber lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    info!("WebSocket events connection ended: {}", session_id);
}

/// Handle subscription request
fn handle_subscription_request(
    request: SubscriptionRequest,
    state: &mut SubscriptionState,
) -> EventNotification {
    match request {
        SubscriptionRequest::Subscribe {
            events,
            execution_id,
        } => {
            for event in &events {
                if !state.events.contains(event) {
                    state.events.push(event.clone());
                }
            }
            state.execution_id = execution_id;
            debug!("Subscribed to events: {:?}", state.events);

            EventNotification::Subscribed {
                events: state.events.clone(),
            }
        }
        SubscriptionRequest::Unsubscribe { events } => {
            state.events.retain(|e| !events.contains(e));
            debug!("Unsubscribed, remaining: {:?}", state.events);

            EventNotification::Subscribed {
                events: state.events.clone(),
            }
        }
        SubscriptionRequest::Ping => EventNotification::Pong,
    }
}

/// Convert an OrchestratorEvent into an EventNotification
fn convert_event(
    event: &OrchestratorEvent,
    state: &SubscriptionState,
) -> Option<EventNotification> {
    let now = Utc::now();
    let exec_id = event.execution_id();

    match event {
        OrchestratorEvent::ExecutionStarted {
            execution_id,
            session_key,
        } => {
            if !state.matches("execution_started", *execution_id) {
                return None;
            }
            Some(EventNotification::ExecutionStarted {
                execution_id: *execution_id,
                session_key: session_key.clone(),
                timestamp: now,
            })
        }
        OrchestratorEvent::ExecutionCompleted { execution_id } => {
            if !state.matches("execution_completed", *execution_id) {
                return None;
            }
            Some(EventNotification::ExecutionCompleted {
                execution_id: *execution_id,
                timestamp: now,
            })
        }
        OrchestratorEvent::ExecutionFailed {
            execution_id,
            error,
        } => {
            if !state.matches("execution_failed", *execution_id) {
                return None;
            }
            Some(EventNotification::ExecutionFailed {
                execution_id: *execution_id,
                error: error.clone(),
                timestamp: now,
            })
        }
        OrchestratorEvent::ExecutionCancelled { execution_id } => {
            if !state.matches("execution_cancelled", *execution_id) {
                return None;
            }
            Some(EventNotification::ExecutionCancelled {
                execution_id: *execution_id,
                timestamp: now,
            })
        }
        OrchestratorEvent::ToolStarted {
            execution_id,
            tool_name,
            tool_call_id,
        } => {
            if !state.matches("tool_started", *execution_id) {
                return None;
            }
            Some(EventNotification::ToolCallStarted {
                execution_id: *execution_id,
                tool_name: tool_name.clone(),
                tool_call_id: tool_call_id.clone(),
                timestamp: now,
            })
        }
        OrchestratorEvent::ToolCompleted {
            execution_id,
            tool_call_id,
            tool_name,
            success,
            duration_ms,
        } => {
            if !state.matches("tool_completed", *execution_id) {
                return None;
            }
            Some(EventNotification::ToolCallCompleted {
                execution_id: *execution_id,
                tool_name: tool_name.clone(),
                tool_call_id: tool_call_id.clone(),
                success: *success,
                duration_ms: *duration_ms,
                timestamp: now,
            })
        }
        OrchestratorEvent::PlanningStarted {
            execution_id,
            iteration,
        } => {
            if !state.matches("planning_started", *execution_id) {
                return None;
            }
            Some(EventNotification::PlanningStarted {
                execution_id: *execution_id,
                iteration: *iteration,
                timestamp: now,
            })
        }
        OrchestratorEvent::ChatDelta {
            execution_id,
            delta,
            is_final,
        } => {
            if !state.matches("chat_delta", *execution_id) {
                return None;
            }
            Some(EventNotification::ChatDelta {
                execution_id: *execution_id,
                delta: delta.clone(),
                is_final: *is_final,
                timestamp: now,
            })
        }
        OrchestratorEvent::ApprovalRequired {
            execution_id,
            request_id,
        } => {
            if !state.matches("approval_required", exec_id) {
                return None;
            }
            Some(EventNotification::ApprovalRequired {
                execution_id: *execution_id,
                request_id: *request_id,
                timestamp: now,
            })
        }
        OrchestratorEvent::A2aMessageSent { .. } => None,
        OrchestratorEvent::QuotaWarning { .. } => None,
    }
}
