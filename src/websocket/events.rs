//! Events WebSocket handler
//!
//! Provides real-time event stream functionality.
//! Connected to EventBus for streaming OrchestratorEvent notifications.

use std::sync::Arc;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{debug, error, info};
use uuid::Uuid;

use cratos_core::event_bus::{EventBus, OrchestratorEvent};

use crate::middleware::auth::RequireAuthStrict;

/// Subscription request from client
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubscriptionRequest {
    /// Subscribe to events
    Subscribe {
        /// Event types to subscribe to
        events: Vec<String>,
        /// Optional execution ID to filter
        execution_id: Option<Uuid>,
    },
    /// Unsubscribe from events
    Unsubscribe {
        /// Event types to unsubscribe from
        events: Vec<String>,
    },
    /// Ping for keepalive
    Ping,
}

/// Event notification to client
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventNotification {
    /// Execution started
    ExecutionStarted {
        execution_id: Uuid,
        session_key: String,
        timestamp: DateTime<Utc>,
    },
    /// Execution completed
    ExecutionCompleted {
        execution_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    /// Execution failed
    ExecutionFailed {
        execution_id: Uuid,
        error: String,
        timestamp: DateTime<Utc>,
    },
    /// Execution cancelled
    ExecutionCancelled {
        execution_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    /// Tool call started
    ToolCallStarted {
        execution_id: Uuid,
        tool_name: String,
        tool_call_id: String,
        timestamp: DateTime<Utc>,
    },
    /// Tool call completed
    ToolCallCompleted {
        execution_id: Uuid,
        tool_name: String,
        tool_call_id: String,
        success: bool,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    /// Planning started
    PlanningStarted {
        execution_id: Uuid,
        iteration: usize,
        timestamp: DateTime<Utc>,
    },
    /// Chat delta (streaming text)
    ChatDelta {
        execution_id: Uuid,
        delta: String,
        is_final: bool,
        timestamp: DateTime<Utc>,
    },
    /// Approval required
    ApprovalRequired {
        execution_id: Uuid,
        request_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    /// Subscription confirmed
    Subscribed { events: Vec<String> },
    /// Error notification
    Error {
        message: String,
        code: Option<String>,
    },
    /// Pong response
    Pong,
    /// Connection established
    Connected { session_id: Uuid },
}

/// Client subscription state
struct SubscriptionState {
    /// Subscribed event types (empty = all)
    events: Vec<String>,
    /// Optional execution ID filter
    execution_id: Option<Uuid>,
}

impl SubscriptionState {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            execution_id: None,
        }
    }

    /// Check if a given event type matches the subscription filter
    fn matches(&self, event_type: &str, exec_id: Uuid) -> bool {
        // If filtering by execution ID, check it
        if let Some(filter_id) = self.execution_id {
            if exec_id != filter_id {
                return false;
            }
        }
        // If no event types subscribed, nothing matches (must explicitly subscribe)
        if self.events.is_empty() {
            return false;
        }
        // Check event type match
        self.events.iter().any(|e| e == event_type || e == "*")
    }
}

/// WebSocket upgrade handler (requires strict authentication — never bypassed)
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

    // Message handling loop: multiplex client requests and EventBus events
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

/// Convert an OrchestratorEvent into an EventNotification, respecting subscription filters.
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
        OrchestratorEvent::A2aMessageSent { .. } => {
            // A2A events are not forwarded to the events WS
            None
        }
        OrchestratorEvent::QuotaWarning { .. } => {
            // QuotaWarning events are handled via notify_chat_id in Telegram,
            // not forwarded to the general events WS
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe() {
        let mut state = SubscriptionState::new();
        let request = SubscriptionRequest::Subscribe {
            events: vec!["execution_started".to_string(), "tool_started".to_string()],
            execution_id: None,
        };

        let response = handle_subscription_request(request, &mut state);
        if let EventNotification::Subscribed { events } = response {
            assert_eq!(events.len(), 2);
            assert!(events.contains(&"execution_started".to_string()));
        } else {
            panic!("Expected Subscribed");
        }
    }

    #[test]
    fn test_unsubscribe() {
        let mut state = SubscriptionState::new();
        state.events = vec![
            "execution_started".to_string(),
            "tool_started".to_string(),
            "chat_delta".to_string(),
        ];
        let request = SubscriptionRequest::Unsubscribe {
            events: vec!["tool_started".to_string()],
        };

        let response = handle_subscription_request(request, &mut state);
        if let EventNotification::Subscribed { events } = response {
            assert_eq!(events.len(), 2);
            assert!(!events.contains(&"tool_started".to_string()));
        } else {
            panic!("Expected Subscribed");
        }
    }

    #[test]
    fn test_ping() {
        let mut state = SubscriptionState::new();
        let request = SubscriptionRequest::Ping;

        let response = handle_subscription_request(request, &mut state);
        assert!(matches!(response, EventNotification::Pong));
    }

    #[test]
    fn test_subscription_filter_matches() {
        let mut state = SubscriptionState::new();
        state.events = vec!["execution_started".to_string()];

        let id = Uuid::new_v4();
        assert!(state.matches("execution_started", id));
        assert!(!state.matches("tool_started", id));
    }

    #[test]
    fn test_subscription_wildcard() {
        let mut state = SubscriptionState::new();
        state.events = vec!["*".to_string()];

        let id = Uuid::new_v4();
        assert!(state.matches("execution_started", id));
        assert!(state.matches("tool_completed", id));
    }

    #[test]
    fn test_subscription_execution_id_filter() {
        let target_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();

        let mut state = SubscriptionState::new();
        state.events = vec!["*".to_string()];
        state.execution_id = Some(target_id);

        assert!(state.matches("execution_started", target_id));
        assert!(!state.matches("execution_started", other_id));
    }

    #[test]
    fn test_empty_subscription_matches_nothing() {
        let state = SubscriptionState::new();
        let id = Uuid::new_v4();
        assert!(!state.matches("execution_started", id));
    }

    #[test]
    fn test_convert_execution_started() {
        let id = Uuid::new_v4();
        let mut state = SubscriptionState::new();
        state.events = vec!["execution_started".to_string()];

        let event = OrchestratorEvent::ExecutionStarted {
            execution_id: id,
            session_key: "ws:1:1".to_string(),
        };

        let notification = convert_event(&event, &state);
        assert!(notification.is_some());
        if let Some(EventNotification::ExecutionStarted {
            execution_id,
            session_key,
            ..
        }) = notification
        {
            assert_eq!(execution_id, id);
            assert_eq!(session_key, "ws:1:1");
        } else {
            panic!("Expected ExecutionStarted");
        }
    }

    #[test]
    fn test_convert_filtered_out() {
        let id = Uuid::new_v4();
        let mut state = SubscriptionState::new();
        state.events = vec!["tool_started".to_string()];

        let event = OrchestratorEvent::ExecutionStarted {
            execution_id: id,
            session_key: "ws:1:1".to_string(),
        };

        let notification = convert_event(&event, &state);
        assert!(notification.is_none());
    }
}
