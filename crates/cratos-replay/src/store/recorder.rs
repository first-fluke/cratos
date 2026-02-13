//! EventRecorder - Convenient event creation during execution

use super::events::{get_next_sequence_num, record_event};
use super::event_store::EventStore;
use crate::error::Result;
use crate::event::{Event, EventType};
use uuid::Uuid;

/// Event recorder for convenient event creation during execution
pub struct EventRecorder {
    store: EventStore,
    execution_id: Uuid,
    sequence_counter: std::sync::atomic::AtomicI32,
}

impl EventRecorder {
    /// Create a new event recorder for an execution
    pub fn new(store: EventStore, execution_id: Uuid) -> Self {
        Self {
            store,
            execution_id,
            sequence_counter: std::sync::atomic::AtomicI32::new(1),
        }
    }

    /// Create a new event recorder, initializing from existing events
    pub async fn new_from_existing(store: EventStore, execution_id: Uuid) -> Result<Self> {
        let next_seq = get_next_sequence_num(&store, execution_id).await?;
        Ok(Self {
            store,
            execution_id,
            sequence_counter: std::sync::atomic::AtomicI32::new(next_seq),
        })
    }

    /// Get the execution ID
    #[must_use]
    pub fn execution_id(&self) -> Uuid {
        self.execution_id
    }

    /// Record an event with automatic sequence numbering
    pub async fn record(&self, event_type: EventType, payload: serde_json::Value) -> Result<Event> {
        let seq = self
            .sequence_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let event = Event::new(self.execution_id, seq, event_type).with_payload(payload);
        record_event(&self.store, &event).await?;
        Ok(event)
    }

    /// Record an event with duration
    pub async fn record_with_duration(
        &self,
        event_type: EventType,
        payload: serde_json::Value,
        duration_ms: i32,
    ) -> Result<Event> {
        let seq = self
            .sequence_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let event = Event::new(self.execution_id, seq, event_type)
            .with_payload(payload)
            .with_duration(duration_ms);
        record_event(&self.store, &event).await?;
        Ok(event)
    }

    /// Record a child event
    pub async fn record_child(
        &self,
        parent_id: Uuid,
        event_type: EventType,
        payload: serde_json::Value,
    ) -> Result<Event> {
        let seq = self
            .sequence_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let event = Event::new(self.execution_id, seq, event_type)
            .with_payload(payload)
            .with_parent(parent_id);
        record_event(&self.store, &event).await?;
        Ok(event)
    }

    /// Record a user input event
    pub async fn record_user_input(&self, text: &str) -> Result<Event> {
        self.record(
            EventType::UserInput,
            serde_json::json!({
                "text": text,
                "attachments": []
            }),
        )
        .await
    }

    /// Record an LLM request event
    pub async fn record_llm_request(
        &self,
        provider: &str,
        model: &str,
        message_count: usize,
        tool_names: &[String],
    ) -> Result<Event> {
        self.record(
            EventType::LlmRequest,
            serde_json::json!({
                "provider": provider,
                "model": model,
                "message_count": message_count,
                "has_tools": !tool_names.is_empty(),
                "tool_names": tool_names,
            }),
        )
        .await
    }

    /// Record an LLM response event
    pub async fn record_llm_response(
        &self,
        provider: &str,
        model: &str,
        content_preview: &str,
        has_tool_calls: bool,
        tokens: Option<(u32, u32, u32)>,
        duration_ms: i32,
    ) -> Result<Event> {
        let tokens_json = tokens.map(|(prompt, completion, total)| {
            serde_json::json!({
                "prompt_tokens": prompt,
                "completion_tokens": completion,
                "total_tokens": total,
            })
        });

        self.record_with_duration(
            EventType::LlmResponse,
            serde_json::json!({
                "provider": provider,
                "model": model,
                "content_preview": content_preview,
                "has_tool_calls": has_tool_calls,
                "tokens": tokens_json,
            }),
            duration_ms,
        )
        .await
    }

    /// Record a tool call event
    pub async fn record_tool_call(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        risk_level: &str,
        requires_approval: bool,
    ) -> Result<Event> {
        self.record(
            EventType::ToolCall,
            serde_json::json!({
                "tool_name": tool_name,
                "input": input,
                "risk_level": risk_level,
                "requires_approval": requires_approval,
            }),
        )
        .await
    }

    /// Record a tool result event
    pub async fn record_tool_result(
        &self,
        tool_name: &str,
        success: bool,
        output: Option<serde_json::Value>,
        error: Option<&str>,
        duration_ms: i32,
    ) -> Result<Event> {
        self.record_with_duration(
            EventType::ToolResult,
            serde_json::json!({
                "tool_name": tool_name,
                "success": success,
                "output": output,
                "error": error,
            }),
            duration_ms,
        )
        .await
    }

    /// Record a final response event
    pub async fn record_final_response(&self, response: &str) -> Result<Event> {
        self.record(
            EventType::FinalResponse,
            serde_json::json!({
                "response": response,
            }),
        )
        .await
    }

    /// Record an error event
    pub async fn record_error(
        &self,
        code: &str,
        message: &str,
        recoverable: bool,
    ) -> Result<Event> {
        self.record(
            EventType::Error,
            serde_json::json!({
                "code": code,
                "message": message,
                "stack_trace": null,
                "recoverable": recoverable,
            }),
        )
        .await
    }
}
