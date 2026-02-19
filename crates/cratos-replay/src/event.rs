//! Event - Event types and schemas for the replay system
//!
//! This module defines all event types that can be recorded during execution.
//! Events form an immutable audit log that enables replay functionality.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Event types for the replay system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// User input received
    UserInput,
    /// Execution plan created
    PlanCreated,
    /// LLM request sent
    LlmRequest,
    /// LLM response received
    LlmResponse,
    /// Tool invocation started
    ToolCall,
    /// Tool execution result
    ToolResult,
    /// Final response generated
    FinalResponse,
    /// Error occurred
    Error,
    /// Approval requested for high-risk operation
    ApprovalRequested,
    /// Approval granted
    ApprovalGranted,
    /// Approval denied
    ApprovalDenied,
    /// Execution cancelled
    Cancelled,
    /// Context updated (memory/session)
    ContextUpdated,
}

impl EventType {
    /// Returns the string representation of the event type
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserInput => "user_input",
            Self::PlanCreated => "plan_created",
            Self::LlmRequest => "llm_request",
            Self::LlmResponse => "llm_response",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
            Self::FinalResponse => "final_response",
            Self::Error => "error",
            Self::ApprovalRequested => "approval_requested",
            Self::ApprovalGranted => "approval_granted",
            Self::ApprovalDenied => "approval_denied",
            Self::Cancelled => "cancelled",
            Self::ContextUpdated => "context_updated",
        }
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for EventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user_input" => Ok(Self::UserInput),
            "plan_created" => Ok(Self::PlanCreated),
            "llm_request" => Ok(Self::LlmRequest),
            "llm_response" => Ok(Self::LlmResponse),
            "tool_call" => Ok(Self::ToolCall),
            "tool_result" => Ok(Self::ToolResult),
            "final_response" => Ok(Self::FinalResponse),
            "error" => Ok(Self::Error),
            "approval_requested" => Ok(Self::ApprovalRequested),
            "approval_granted" => Ok(Self::ApprovalGranted),
            "approval_denied" => Ok(Self::ApprovalDenied),
            "cancelled" => Ok(Self::Cancelled),
            "context_updated" => Ok(Self::ContextUpdated),
            _ => Err(format!("unknown event type: {s}")),
        }
    }
}

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Execution is pending
    Pending,
    /// Execution is running
    Running,
    /// Execution completed successfully
    Completed,
    /// Execution failed
    Failed,
    /// Execution was cancelled
    Cancelled,
}

impl ExecutionStatus {
    /// Returns the string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Check if the execution is terminal (completed, failed, or cancelled)
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ExecutionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("unknown execution status: {s}")),
        }
    }
}

/// An execution record representing a complete user request lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    /// Unique identifier
    pub id: Uuid,

    /// Channel type (telegram, slack, api)
    pub channel_type: String,

    /// Channel identifier (chat/channel ID)
    pub channel_id: String,

    /// User who initiated the execution
    pub user_id: String,

    /// Session identifier
    pub session_id: Option<String>,

    /// Thread ID for reply context
    pub thread_id: Option<String>,

    /// Current status
    pub status: ExecutionStatus,

    /// When the execution started
    pub started_at: DateTime<Utc>,

    /// When the execution completed (if finished)
    pub completed_at: Option<DateTime<Utc>>,

    /// Original user input
    pub input_text: String,

    /// Final output (if completed)
    pub output_text: Option<String>,

    /// Additional metadata
    pub metadata: serde_json::Value,

    /// When the record was created
    pub created_at: DateTime<Utc>,

    /// When the record was last updated
    pub updated_at: DateTime<Utc>,
}

impl Execution {
    /// Create a new execution
    #[must_use]
    pub fn new(
        channel_type: impl Into<String>,
        channel_id: impl Into<String>,
        user_id: impl Into<String>,
        input_text: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            channel_type: channel_type.into(),
            channel_id: channel_id.into(),
            user_id: user_id.into(),
            session_id: None,
            thread_id: None,
            status: ExecutionStatus::Pending,
            started_at: now,
            completed_at: None,
            input_text: input_text.into(),
            output_text: None,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    /// Set the session ID
    #[must_use]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set the thread ID
    #[must_use]
    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Set metadata
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Mark execution as running
    pub fn mark_running(&mut self) {
        self.status = ExecutionStatus::Running;
        self.updated_at = Utc::now();
    }

    /// Mark execution as completed
    pub fn mark_completed(&mut self, output: impl Into<String>) {
        self.status = ExecutionStatus::Completed;
        self.output_text = Some(output.into());
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Mark execution as failed
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = ExecutionStatus::Failed;
        self.output_text = Some(error.into());
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Mark execution as cancelled
    pub fn mark_cancelled(&mut self) {
        self.status = ExecutionStatus::Cancelled;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}

/// An event in the execution timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique identifier
    pub id: Uuid,

    /// Execution this event belongs to
    pub execution_id: Uuid,

    /// Sequence number within the execution
    pub sequence_num: i32,

    /// Type of event
    pub event_type: EventType,

    /// Event-specific payload
    pub payload: serde_json::Value,

    /// When the event occurred
    pub timestamp: DateTime<Utc>,

    /// Duration in milliseconds (if applicable)
    pub duration_ms: Option<i32>,

    /// Parent event ID (for nested events)
    pub parent_event_id: Option<Uuid>,

    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl Event {
    /// Create a new event
    #[must_use]
    pub fn new(execution_id: Uuid, sequence_num: i32, event_type: EventType) -> Self {
        Self {
            id: Uuid::new_v4(),
            execution_id,
            sequence_num,
            event_type,
            payload: serde_json::json!({}),
            timestamp: Utc::now(),
            duration_ms: None,
            parent_event_id: None,
            metadata: serde_json::json!({}),
        }
    }

    /// Set the payload
    #[must_use]
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    /// Set the duration
    #[must_use]
    pub fn with_duration(mut self, duration_ms: i32) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Set the parent event
    #[must_use]
    pub fn with_parent(mut self, parent_id: Uuid) -> Self {
        self.parent_event_id = Some(parent_id);
        self
    }

    /// Set metadata
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Payload for UserInput events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInputPayload {
    /// The user's message
    pub text: String,
    /// Attachments (if any)
    #[serde(default)]
    pub attachments: Vec<AttachmentInfo>,
}

/// Information about an attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    /// Attachment type (file, image, audio, video)
    pub attachment_type: String,
    /// Filename or identifier
    pub name: String,
    /// Size in bytes
    pub size: Option<u64>,
    /// MIME type
    pub mime_type: Option<String>,
}

/// Payload for LlmRequest events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequestPayload {
    /// Provider name
    pub provider: String,
    /// Model name
    pub model: String,
    /// Messages sent (simplified)
    pub message_count: usize,
    /// Whether tools are included
    pub has_tools: bool,
    /// Tool names (if any)
    #[serde(default)]
    pub tool_names: Vec<String>,
}

/// Payload for LlmResponse events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponsePayload {
    /// Provider name
    pub provider: String,
    /// Model name
    pub model: String,
    /// Response content (truncated for storage)
    pub content_preview: String,
    /// Whether a tool call was requested
    pub has_tool_calls: bool,
    /// Token usage
    pub tokens: Option<TokenUsage>,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Prompt tokens
    pub prompt_tokens: u32,
    /// Completion tokens
    pub completion_tokens: u32,
    /// Total tokens
    pub total_tokens: u32,
}

/// Payload for ToolCall events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallPayload {
    /// Tool name
    pub tool_name: String,
    /// Tool input (may be truncated)
    pub input: serde_json::Value,
    /// Risk level
    pub risk_level: String,
    /// Whether approval was required
    pub requires_approval: bool,
}

/// Payload for ToolResult events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultPayload {
    /// Tool name
    pub tool_name: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Output (may be truncated)
    pub output: Option<serde_json::Value>,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Payload for Error events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
    /// Stack trace (if available)
    pub stack_trace: Option<String>,
    /// Whether the error is recoverable
    pub recoverable: bool,
}

/// Payload for PlanCreated events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanCreatedPayload {
    /// Plan steps
    pub steps: Vec<PlanStep>,
    /// Estimated tool calls
    pub estimated_tool_calls: usize,
}

/// A step in the execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Step number
    pub step_num: usize,
    /// Description
    pub description: String,
    /// Tool to use (if any)
    pub tool: Option<String>,
}

/// Timeline entry for displaying execution history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    /// Event ID
    pub event_id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: EventType,
    /// Human-readable summary
    pub summary: String,
    /// Duration (if applicable)
    pub duration_ms: Option<i32>,
    /// Whether this event has children
    pub has_children: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_roundtrip() {
        for event_type in [
            EventType::UserInput,
            EventType::PlanCreated,
            EventType::LlmRequest,
            EventType::LlmResponse,
            EventType::ToolCall,
            EventType::ToolResult,
            EventType::FinalResponse,
            EventType::Error,
        ] {
            let s = event_type.to_string();
            let parsed: EventType = s.parse().unwrap();
            assert_eq!(event_type, parsed);
        }
    }

    #[test]
    fn test_execution_status_terminal() {
        assert!(!ExecutionStatus::Pending.is_terminal());
        assert!(!ExecutionStatus::Running.is_terminal());
        assert!(ExecutionStatus::Completed.is_terminal());
        assert!(ExecutionStatus::Failed.is_terminal());
        assert!(ExecutionStatus::Cancelled.is_terminal());
    }

    #[test]
    fn test_execution_lifecycle() {
        let mut exec = Execution::new("telegram", "12345", "user1", "Hello");

        assert_eq!(exec.status, ExecutionStatus::Pending);
        assert!(exec.completed_at.is_none());

        exec.mark_running();
        assert_eq!(exec.status, ExecutionStatus::Running);

        exec.mark_completed("Done!");
        assert_eq!(exec.status, ExecutionStatus::Completed);
        assert!(exec.completed_at.is_some());
        assert_eq!(exec.output_text, Some("Done!".to_string()));
    }

    #[test]
    fn test_event_creation() {
        let execution_id = Uuid::new_v4();
        let event = Event::new(execution_id, 1, EventType::UserInput)
            .with_payload(serde_json::json!({"text": "hello"}))
            .with_duration(100);

        assert_eq!(event.execution_id, execution_id);
        assert_eq!(event.sequence_num, 1);
        assert_eq!(event.event_type, EventType::UserInput);
        assert_eq!(event.duration_ms, Some(100));
    }

    #[test]
    fn test_event_type_serialization() {
        let event_type = EventType::ToolCall;
        let json = serde_json::to_string(&event_type).unwrap();
        assert_eq!(json, r#""tool_call""#);

        let parsed: EventType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, EventType::ToolCall);
    }
}
