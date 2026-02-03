//! Replay Event Types
//!
//! This module defines event types for the canvas replay system.
//! Events are recorded for audit logging and enable replay functionality.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::document::CanvasBlock;
use crate::protocol::UpdateSource;

/// Canvas event types for replay
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasEventType {
    /// Session created
    SessionCreated,
    /// Session joined by user
    SessionJoined,
    /// Session left by user
    SessionLeft,
    /// Session closed
    SessionClosed,
    /// Block added
    BlockAdded,
    /// Block updated
    BlockUpdated,
    /// Block deleted
    BlockDeleted,
    /// Block moved
    BlockMoved,
    /// AI request started
    AiRequestStarted,
    /// AI response chunk received
    AiChunkReceived,
    /// AI response completed
    AiCompleted,
    /// AI request cancelled
    AiCancelled,
    /// AI error occurred
    AiError,
    /// Code execution started
    CodeExecutionStarted,
    /// Code execution output
    CodeExecutionOutput,
    /// Code execution completed
    CodeExecutionCompleted,
    /// Document saved
    DocumentSaved,
    /// Document exported
    DocumentExported,
}

impl CanvasEventType {
    /// Get the string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SessionCreated => "session_created",
            Self::SessionJoined => "session_joined",
            Self::SessionLeft => "session_left",
            Self::SessionClosed => "session_closed",
            Self::BlockAdded => "block_added",
            Self::BlockUpdated => "block_updated",
            Self::BlockDeleted => "block_deleted",
            Self::BlockMoved => "block_moved",
            Self::AiRequestStarted => "ai_request_started",
            Self::AiChunkReceived => "ai_chunk_received",
            Self::AiCompleted => "ai_completed",
            Self::AiCancelled => "ai_cancelled",
            Self::AiError => "ai_error",
            Self::CodeExecutionStarted => "code_execution_started",
            Self::CodeExecutionOutput => "code_execution_output",
            Self::CodeExecutionCompleted => "code_execution_completed",
            Self::DocumentSaved => "document_saved",
            Self::DocumentExported => "document_exported",
        }
    }

    /// Check if this is an AI-related event
    #[must_use]
    pub fn is_ai_event(&self) -> bool {
        matches!(
            self,
            Self::AiRequestStarted
                | Self::AiChunkReceived
                | Self::AiCompleted
                | Self::AiCancelled
                | Self::AiError
        )
    }

    /// Check if this is a block-related event
    #[must_use]
    pub fn is_block_event(&self) -> bool {
        matches!(
            self,
            Self::BlockAdded | Self::BlockUpdated | Self::BlockDeleted | Self::BlockMoved
        )
    }
}

impl std::fmt::Display for CanvasEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A canvas event for replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasEvent {
    /// Unique event identifier
    pub id: Uuid,

    /// Session this event belongs to
    pub session_id: Uuid,

    /// Sequence number within the session
    pub sequence_num: i64,

    /// Event type
    pub event_type: CanvasEventType,

    /// User who triggered the event
    pub user_id: String,

    /// Event-specific payload
    pub payload: serde_json::Value,

    /// When the event occurred
    pub timestamp: DateTime<Utc>,

    /// Duration in milliseconds (for timed events)
    pub duration_ms: Option<i64>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl CanvasEvent {
    /// Create a new event
    #[must_use]
    pub fn new(
        session_id: Uuid,
        sequence_num: i64,
        event_type: CanvasEventType,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            sequence_num,
            event_type,
            user_id: user_id.into(),
            payload: serde_json::json!({}),
            timestamp: Utc::now(),
            duration_ms: None,
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
    pub fn with_duration(mut self, duration_ms: i64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Set metadata
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

// ============================================================================
// Event Payloads
// ============================================================================

/// Payload for session created event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreatedPayload {
    /// Document title
    pub title: String,
    /// Associated execution ID (if any)
    pub execution_id: Option<Uuid>,
}

/// Payload for session joined event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionJoinedPayload {
    /// Connection ID
    pub connection_id: Uuid,
    /// Client info (user agent, etc.)
    #[serde(default)]
    pub client_info: serde_json::Value,
}

/// Payload for block added event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockAddedPayload {
    /// The new block
    pub block: CanvasBlock,
    /// Index where block was inserted
    pub index: usize,
    /// Source of the addition
    pub source: UpdateSource,
}

/// Payload for block updated event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockUpdatedPayload {
    /// Block ID
    pub block_id: Uuid,
    /// Previous content (for undo)
    pub previous_content: String,
    /// New content
    pub new_content: String,
    /// Source of the update
    pub source: UpdateSource,
}

/// Payload for block deleted event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDeletedPayload {
    /// The deleted block (for undo)
    pub block: CanvasBlock,
    /// Index where block was located
    pub index: usize,
}

/// Payload for block moved event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMovedPayload {
    /// Block ID
    pub block_id: Uuid,
    /// Previous index
    pub from_index: usize,
    /// New index
    pub to_index: usize,
}

/// Payload for AI request started event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestStartedPayload {
    /// User prompt
    pub prompt: String,
    /// Context block IDs
    pub context_blocks: Vec<Uuid>,
    /// Target block ID
    pub target_block_id: Uuid,
    /// Model used
    pub model: Option<String>,
}

/// Payload for AI chunk received event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChunkReceivedPayload {
    /// Target block ID
    pub block_id: Uuid,
    /// Chunk content
    pub chunk: String,
    /// Accumulated content length
    pub total_length: usize,
}

/// Payload for AI completed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCompletedPayload {
    /// Target block ID
    pub block_id: Uuid,
    /// Total content length
    pub content_length: usize,
    /// Tokens used
    pub tokens_used: Option<u32>,
    /// Model used
    pub model: Option<String>,
}

/// Payload for AI error event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiErrorPayload {
    /// Error message
    pub error: String,
    /// Error code
    pub code: Option<String>,
    /// Target block ID (if any)
    pub target_block_id: Option<Uuid>,
}

/// Payload for code execution started event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutionStartedPayload {
    /// Block ID being executed
    pub block_id: Uuid,
    /// Programming language
    pub language: String,
    /// Code being executed
    pub code: String,
}

/// Payload for code execution output event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutionOutputPayload {
    /// Block ID
    pub block_id: Uuid,
    /// Output text
    pub output: String,
    /// Whether this is stderr
    pub is_error: bool,
}

/// Payload for code execution completed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutionCompletedPayload {
    /// Block ID
    pub block_id: Uuid,
    /// Exit code
    pub exit_code: i32,
    /// Total output length
    pub output_length: usize,
}

/// Payload for document saved event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSavedPayload {
    /// Document ID
    pub document_id: Uuid,
    /// Block count
    pub block_count: usize,
    /// Storage location
    pub storage_location: String,
}

/// Payload for document exported event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentExportedPayload {
    /// Export format (markdown, html, pdf)
    pub format: String,
    /// File size in bytes
    pub file_size: Option<u64>,
}

// ============================================================================
// Event Recording
// ============================================================================

/// Event recorder for canvas sessions
pub struct CanvasEventRecorder {
    session_id: Uuid,
    user_id: String,
    sequence_counter: std::sync::atomic::AtomicI64,
    events: std::sync::Arc<tokio::sync::RwLock<Vec<CanvasEvent>>>,
}

impl CanvasEventRecorder {
    /// Create a new event recorder
    #[must_use]
    pub fn new(session_id: Uuid, user_id: impl Into<String>) -> Self {
        Self {
            session_id,
            user_id: user_id.into(),
            sequence_counter: std::sync::atomic::AtomicI64::new(1),
            events: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Get the session ID
    #[must_use]
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// Get the user ID
    #[must_use]
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Record an event
    pub async fn record(&self, event_type: CanvasEventType, payload: serde_json::Value) {
        let seq = self
            .sequence_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let event =
            CanvasEvent::new(self.session_id, seq, event_type, &self.user_id).with_payload(payload);

        let mut events = self.events.write().await;
        events.push(event);
    }

    /// Record an event with duration
    pub async fn record_with_duration(
        &self,
        event_type: CanvasEventType,
        payload: serde_json::Value,
        duration_ms: i64,
    ) {
        let seq = self
            .sequence_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let event = CanvasEvent::new(self.session_id, seq, event_type, &self.user_id)
            .with_payload(payload)
            .with_duration(duration_ms);

        let mut events = self.events.write().await;
        events.push(event);
    }

    /// Get all recorded events
    pub async fn get_events(&self) -> Vec<CanvasEvent> {
        let events = self.events.read().await;
        events.clone()
    }

    /// Get event count
    pub async fn event_count(&self) -> usize {
        let events = self.events.read().await;
        events.len()
    }

    /// Clear all events
    pub async fn clear(&self) {
        let mut events = self.events.write().await;
        events.clear();
    }

    /// Record a block added event
    pub async fn record_block_added(
        &self,
        block: &CanvasBlock,
        index: usize,
        source: UpdateSource,
    ) {
        self.record(
            CanvasEventType::BlockAdded,
            serde_json::to_value(BlockAddedPayload {
                block: block.clone(),
                index,
                source,
            })
            .unwrap_or_default(),
        )
        .await;
    }

    /// Record a block updated event
    pub async fn record_block_updated(
        &self,
        block_id: Uuid,
        previous_content: &str,
        new_content: &str,
        source: UpdateSource,
    ) {
        self.record(
            CanvasEventType::BlockUpdated,
            serde_json::to_value(BlockUpdatedPayload {
                block_id,
                previous_content: previous_content.to_string(),
                new_content: new_content.to_string(),
                source,
            })
            .unwrap_or_default(),
        )
        .await;
    }

    /// Record a block deleted event
    pub async fn record_block_deleted(&self, block: &CanvasBlock, index: usize) {
        self.record(
            CanvasEventType::BlockDeleted,
            serde_json::to_value(BlockDeletedPayload {
                block: block.clone(),
                index,
            })
            .unwrap_or_default(),
        )
        .await;
    }

    /// Record AI started event
    pub async fn record_ai_started(
        &self,
        prompt: &str,
        context_blocks: &[Uuid],
        target_block_id: Uuid,
        model: Option<&str>,
    ) {
        self.record(
            CanvasEventType::AiRequestStarted,
            serde_json::to_value(AiRequestStartedPayload {
                prompt: prompt.to_string(),
                context_blocks: context_blocks.to_vec(),
                target_block_id,
                model: model.map(String::from),
            })
            .unwrap_or_default(),
        )
        .await;
    }

    /// Record AI completed event
    pub async fn record_ai_completed(
        &self,
        block_id: Uuid,
        content_length: usize,
        tokens_used: Option<u32>,
        model: Option<&str>,
        duration_ms: i64,
    ) {
        self.record_with_duration(
            CanvasEventType::AiCompleted,
            serde_json::to_value(AiCompletedPayload {
                block_id,
                content_length,
                tokens_used,
                model: model.map(String::from),
            })
            .unwrap_or_default(),
            duration_ms,
        )
        .await;
    }

    /// Record AI error event
    pub async fn record_ai_error(
        &self,
        error: &str,
        code: Option<&str>,
        target_block_id: Option<Uuid>,
    ) {
        self.record(
            CanvasEventType::AiError,
            serde_json::to_value(AiErrorPayload {
                error: error.to_string(),
                code: code.map(String::from),
                target_block_id,
            })
            .unwrap_or_default(),
        )
        .await;
    }
}

// ============================================================================
// Timeline Entry
// ============================================================================

/// Timeline entry for displaying event history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasTimelineEntry {
    /// Event ID
    pub event_id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: CanvasEventType,
    /// Human-readable summary
    pub summary: String,
    /// Duration (if applicable)
    pub duration_ms: Option<i64>,
    /// User who triggered the event
    pub user_id: String,
}

impl From<&CanvasEvent> for CanvasTimelineEntry {
    fn from(event: &CanvasEvent) -> Self {
        let summary = create_event_summary(event);
        Self {
            event_id: event.id,
            timestamp: event.timestamp,
            event_type: event.event_type,
            summary,
            duration_ms: event.duration_ms,
            user_id: event.user_id.clone(),
        }
    }
}

/// Create a human-readable summary for an event
fn create_event_summary(event: &CanvasEvent) -> String {
    match event.event_type {
        CanvasEventType::SessionCreated => "Session created".to_string(),
        CanvasEventType::SessionJoined => "User joined session".to_string(),
        CanvasEventType::SessionLeft => "User left session".to_string(),
        CanvasEventType::SessionClosed => "Session closed".to_string(),
        CanvasEventType::BlockAdded => "Block added".to_string(),
        CanvasEventType::BlockUpdated => "Block updated".to_string(),
        CanvasEventType::BlockDeleted => "Block deleted".to_string(),
        CanvasEventType::BlockMoved => "Block moved".to_string(),
        CanvasEventType::AiRequestStarted => "AI request started".to_string(),
        CanvasEventType::AiChunkReceived => "AI response chunk".to_string(),
        CanvasEventType::AiCompleted => "AI response completed".to_string(),
        CanvasEventType::AiCancelled => "AI request cancelled".to_string(),
        CanvasEventType::AiError => "AI error occurred".to_string(),
        CanvasEventType::CodeExecutionStarted => "Code execution started".to_string(),
        CanvasEventType::CodeExecutionOutput => "Code output".to_string(),
        CanvasEventType::CodeExecutionCompleted => "Code execution completed".to_string(),
        CanvasEventType::DocumentSaved => "Document saved".to_string(),
        CanvasEventType::DocumentExported => "Document exported".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_event_type_as_str() {
        assert_eq!(CanvasEventType::SessionCreated.as_str(), "session_created");
        assert_eq!(CanvasEventType::BlockAdded.as_str(), "block_added");
        assert_eq!(CanvasEventType::AiCompleted.as_str(), "ai_completed");
    }

    #[test]
    fn test_canvas_event_type_is_ai_event() {
        assert!(CanvasEventType::AiRequestStarted.is_ai_event());
        assert!(CanvasEventType::AiCompleted.is_ai_event());
        assert!(!CanvasEventType::BlockAdded.is_ai_event());
    }

    #[test]
    fn test_canvas_event_type_is_block_event() {
        assert!(CanvasEventType::BlockAdded.is_block_event());
        assert!(CanvasEventType::BlockUpdated.is_block_event());
        assert!(!CanvasEventType::AiCompleted.is_block_event());
    }

    #[test]
    fn test_canvas_event_creation() {
        let session_id = Uuid::new_v4();
        let event = CanvasEvent::new(session_id, 1, CanvasEventType::BlockAdded, "user1")
            .with_payload(serde_json::json!({"test": true}))
            .with_duration(100);

        assert_eq!(event.session_id, session_id);
        assert_eq!(event.sequence_num, 1);
        assert_eq!(event.event_type, CanvasEventType::BlockAdded);
        assert_eq!(event.user_id, "user1");
        assert_eq!(event.duration_ms, Some(100));
    }

    #[test]
    fn test_block_added_payload() {
        let block = crate::document::CanvasBlock::markdown("Test");
        let payload = BlockAddedPayload {
            block,
            index: 0,
            source: UpdateSource::User,
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"index\":0"));
        assert!(json.contains("\"source\":\"user\""));
    }

    #[test]
    fn test_ai_request_payload() {
        let payload = AiRequestStartedPayload {
            prompt: "Explain this".to_string(),
            context_blocks: vec![Uuid::new_v4()],
            target_block_id: Uuid::new_v4(),
            model: Some("gpt-4".to_string()),
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"prompt\":\"Explain this\""));
        assert!(json.contains("gpt-4"));
    }

    #[tokio::test]
    async fn test_canvas_event_recorder() {
        let session_id = Uuid::new_v4();
        let recorder = CanvasEventRecorder::new(session_id, "user1");

        assert_eq!(recorder.session_id(), session_id);
        assert_eq!(recorder.user_id(), "user1");

        recorder
            .record(CanvasEventType::SessionCreated, serde_json::json!({}))
            .await;
        recorder
            .record(CanvasEventType::BlockAdded, serde_json::json!({}))
            .await;

        assert_eq!(recorder.event_count().await, 2);

        let events = recorder.get_events().await;
        assert_eq!(events[0].event_type, CanvasEventType::SessionCreated);
        assert_eq!(events[1].event_type, CanvasEventType::BlockAdded);
    }

    #[tokio::test]
    async fn test_canvas_event_recorder_clear() {
        let recorder = CanvasEventRecorder::new(Uuid::new_v4(), "user1");

        recorder
            .record(CanvasEventType::SessionCreated, serde_json::json!({}))
            .await;
        assert_eq!(recorder.event_count().await, 1);

        recorder.clear().await;
        assert_eq!(recorder.event_count().await, 0);
    }

    #[test]
    fn test_canvas_timeline_entry_from_event() {
        let event = CanvasEvent::new(Uuid::new_v4(), 1, CanvasEventType::BlockAdded, "user1");

        let entry = CanvasTimelineEntry::from(&event);
        assert_eq!(entry.event_id, event.id);
        assert_eq!(entry.event_type, CanvasEventType::BlockAdded);
        assert_eq!(entry.summary, "Block added");
    }
}
