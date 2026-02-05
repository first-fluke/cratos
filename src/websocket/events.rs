//! Events WebSocket handler
//!
//! Provides real-time event stream functionality

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use uuid::Uuid;

/// Subscription request from client
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
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
#[allow(dead_code)]
pub enum EventNotification {
    /// Execution started
    ExecutionStarted {
        execution_id: Uuid,
        channel_type: String,
        user_id: String,
        input_text: String,
        timestamp: DateTime<Utc>,
    },
    /// Execution completed
    ExecutionCompleted {
        execution_id: Uuid,
        status: String,
        output_text: Option<String>,
        duration_ms: i64,
        timestamp: DateTime<Utc>,
    },
    /// Tool call started
    ToolCallStarted {
        execution_id: Uuid,
        tool_name: String,
        sequence_num: i32,
        timestamp: DateTime<Utc>,
    },
    /// Tool call completed
    ToolCallCompleted {
        execution_id: Uuid,
        tool_name: String,
        success: bool,
        duration_ms: i64,
        error: Option<String>,
        timestamp: DateTime<Utc>,
    },
    /// LLM request started
    LlmRequestStarted {
        execution_id: Uuid,
        provider: String,
        model: String,
        timestamp: DateTime<Utc>,
    },
    /// LLM response received
    LlmResponseReceived {
        execution_id: Uuid,
        tokens_used: i32,
        duration_ms: i64,
        timestamp: DateTime<Utc>,
    },
    /// Scheduler task executed
    SchedulerTaskExecuted {
        task_id: Uuid,
        task_name: String,
        success: bool,
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

/// WebSocket upgrade handler
pub async fn events_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket) {
    let session_id = Uuid::new_v4();
    info!("WebSocket events connection established: {}", session_id);

    let (mut sender, mut receiver) = socket.split();

    // Send connection established message
    let connected_msg = EventNotification::Connected { session_id };
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        let _ = sender.send(Message::Text(json)).await;
    }

    // Track subscriptions
    let mut subscribed_events: Vec<String> = Vec::new();

    // Message handling loop
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Received subscription request: {}", text);

                match serde_json::from_str::<SubscriptionRequest>(&text) {
                    Ok(request) => {
                        let response = handle_subscription_request(request, &mut subscribed_events);
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
            Ok(Message::Close(_)) => {
                info!("WebSocket events connection closed: {}", session_id);
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

    info!("WebSocket events connection ended: {}", session_id);
}

/// Handle subscription request
fn handle_subscription_request(
    request: SubscriptionRequest,
    subscribed: &mut Vec<String>,
) -> EventNotification {
    match request {
        SubscriptionRequest::Subscribe { events, .. } => {
            // Add new subscriptions
            for event in &events {
                if !subscribed.contains(event) {
                    subscribed.push(event.clone());
                }
            }
            debug!("Subscribed to events: {:?}", subscribed);

            EventNotification::Subscribed {
                events: subscribed.clone(),
            }
        }
        SubscriptionRequest::Unsubscribe { events } => {
            // Remove subscriptions
            subscribed.retain(|e| !events.contains(e));
            debug!("Unsubscribed, remaining: {:?}", subscribed);

            EventNotification::Subscribed {
                events: subscribed.clone(),
            }
        }
        SubscriptionRequest::Ping => EventNotification::Pong,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe() {
        let mut subscribed = Vec::new();
        let request = SubscriptionRequest::Subscribe {
            events: vec!["execution_started".to_string(), "tool_call".to_string()],
            execution_id: None,
        };

        let response = handle_subscription_request(request, &mut subscribed);
        if let EventNotification::Subscribed { events } = response {
            assert_eq!(events.len(), 2);
            assert!(events.contains(&"execution_started".to_string()));
        } else {
            panic!("Expected Subscribed");
        }
    }

    #[test]
    fn test_unsubscribe() {
        let mut subscribed = vec![
            "execution_started".to_string(),
            "tool_call".to_string(),
            "llm_request".to_string(),
        ];
        let request = SubscriptionRequest::Unsubscribe {
            events: vec!["tool_call".to_string()],
        };

        let response = handle_subscription_request(request, &mut subscribed);
        if let EventNotification::Subscribed { events } = response {
            assert_eq!(events.len(), 2);
            assert!(!events.contains(&"tool_call".to_string()));
        } else {
            panic!("Expected Subscribed");
        }
    }

    #[test]
    fn test_ping() {
        let mut subscribed = Vec::new();
        let request = SubscriptionRequest::Ping;

        let response = handle_subscription_request(request, &mut subscribed);
        assert!(matches!(response, EventNotification::Pong));
    }
}
