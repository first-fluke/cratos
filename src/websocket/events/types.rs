use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
pub struct SubscriptionState {
    /// Subscribed event types (empty = all)
    pub events: Vec<String>,
    /// Optional execution ID filter
    pub execution_id: Option<Uuid>,
}

impl SubscriptionState {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            execution_id: None,
        }
    }

    /// Check if a given event type matches the subscription filter
    pub fn matches(&self, event_type: &str, exec_id: Uuid) -> bool {
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
