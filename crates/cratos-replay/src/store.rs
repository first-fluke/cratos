//! Store - Event persistence using PostgreSQL
//!
//! This module provides the storage layer for executions and events.
//! It uses sqlx for async PostgreSQL access.

use crate::error::{Error, Result};
use crate::event::{Event, EventType, Execution, ExecutionStatus};
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::Row;
use tracing::{debug, instrument};
use uuid::Uuid;

/// Trait for event storage backends
///
/// This trait allows different storage implementations (PostgreSQL, in-memory, etc.)
/// to be used interchangeably.
#[async_trait::async_trait]
pub trait EventStoreTrait: Send + Sync {
    /// Append an event to the store
    async fn append(&self, event: Event) -> Result<()>;

    /// Get events for an execution
    async fn get_events(&self, execution_id: Uuid) -> Result<Vec<Event>>;

    /// Get the event store name (for logging)
    fn name(&self) -> &str;
}

/// Event store for persisting executions and events to PostgreSQL
#[derive(Clone)]
pub struct EventStore {
    pool: PgPool,
}

impl EventStore {
    /// Create a new event store with the given connection pool
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the underlying connection pool
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // =========================================================================
    // Execution operations
    // =========================================================================

    /// Create a new execution record
    #[instrument(skip(self, execution), fields(execution_id = %execution.id))]
    pub async fn create_execution(&self, execution: &Execution) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO executions (
                id, channel_type, channel_id, user_id, thread_id,
                status, started_at, completed_at,
                input_text, output_text, metadata, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8,
                $9, $10, $11, $12, $13
            )
            "#,
        )
        .bind(execution.id)
        .bind(&execution.channel_type)
        .bind(&execution.channel_id)
        .bind(&execution.user_id)
        .bind(&execution.thread_id)
        .bind(execution.status.as_str())
        .bind(execution.started_at)
        .bind(execution.completed_at)
        .bind(&execution.input_text)
        .bind(&execution.output_text)
        .bind(&execution.metadata)
        .bind(execution.created_at)
        .bind(execution.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!("Created execution {}", execution.id);
        Ok(())
    }

    /// Get an execution by ID
    #[instrument(skip(self))]
    pub async fn get_execution(&self, id: Uuid) -> Result<Execution> {
        let row = sqlx::query(
            r#"
            SELECT id, channel_type, channel_id, user_id, thread_id,
                   status, started_at, completed_at,
                   input_text, output_text, metadata, created_at, updated_at
            FROM executions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::ExecutionNotFound(id.to_string()))?;

        Self::row_to_execution(row)
    }

    /// Update execution status
    #[instrument(skip(self))]
    pub async fn update_execution_status(
        &self,
        id: Uuid,
        status: ExecutionStatus,
        output_text: Option<&str>,
    ) -> Result<()> {
        let completed_at = if status.is_terminal() {
            Some(Utc::now())
        } else {
            None
        };

        sqlx::query(
            r#"
            UPDATE executions
            SET status = $2, output_text = COALESCE($3, output_text),
                completed_at = COALESCE($4, completed_at), updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status.as_str())
        .bind(output_text)
        .bind(completed_at)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!("Updated execution {} status to {}", id, status);
        Ok(())
    }

    /// List executions for a channel
    #[instrument(skip(self))]
    pub async fn list_executions_by_channel(
        &self,
        channel_type: &str,
        channel_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Execution>> {
        let rows = sqlx::query(
            r#"
            SELECT id, channel_type, channel_id, user_id, thread_id,
                   status, started_at, completed_at,
                   input_text, output_text, metadata, created_at, updated_at
            FROM executions
            WHERE channel_type = $1 AND channel_id = $2
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(channel_type)
        .bind(channel_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_execution).collect()
    }

    /// List executions for a user
    #[instrument(skip(self))]
    pub async fn list_executions_by_user(
        &self,
        user_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Execution>> {
        let rows = sqlx::query(
            r#"
            SELECT id, channel_type, channel_id, user_id, thread_id,
                   status, started_at, completed_at,
                   input_text, output_text, metadata, created_at, updated_at
            FROM executions
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_execution).collect()
    }

    /// List recent executions
    #[instrument(skip(self))]
    pub async fn list_recent_executions(&self, limit: i64) -> Result<Vec<Execution>> {
        let rows = sqlx::query(
            r#"
            SELECT id, channel_type, channel_id, user_id, thread_id,
                   status, started_at, completed_at,
                   input_text, output_text, metadata, created_at, updated_at
            FROM executions
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_execution).collect()
    }

    // =========================================================================
    // Event operations
    // =========================================================================

    /// Record a new event
    #[instrument(skip(self, event), fields(event_id = %event.id, execution_id = %event.execution_id))]
    pub async fn record_event(&self, event: &Event) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO events (
                id, execution_id, sequence_num, event_type,
                payload, timestamp, duration_ms, parent_event_id, metadata
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9
            )
            "#,
        )
        .bind(event.id)
        .bind(event.execution_id)
        .bind(event.sequence_num)
        .bind(event.event_type.as_str())
        .bind(&event.payload)
        .bind(event.timestamp)
        .bind(event.duration_ms)
        .bind(event.parent_event_id)
        .bind(&event.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!(
            "Recorded event {} for execution {}",
            event.id, event.execution_id
        );
        Ok(())
    }

    /// Get an event by ID
    #[instrument(skip(self))]
    pub async fn get_event(&self, id: Uuid) -> Result<Event> {
        let row = sqlx::query(
            r#"
            SELECT id, execution_id, sequence_num, event_type,
                   payload, timestamp, duration_ms, parent_event_id, metadata
            FROM events
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound(id.to_string()))?;

        Self::row_to_event(row)
    }

    /// Get all events for an execution
    #[instrument(skip(self))]
    pub async fn get_execution_events(&self, execution_id: Uuid) -> Result<Vec<Event>> {
        let rows = sqlx::query(
            r#"
            SELECT id, execution_id, sequence_num, event_type,
                   payload, timestamp, duration_ms, parent_event_id, metadata
            FROM events
            WHERE execution_id = $1
            ORDER BY sequence_num ASC
            "#,
        )
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_event).collect()
    }

    /// Get events by type for an execution
    #[instrument(skip(self))]
    pub async fn get_events_by_type(
        &self,
        execution_id: Uuid,
        event_type: EventType,
    ) -> Result<Vec<Event>> {
        let rows = sqlx::query(
            r#"
            SELECT id, execution_id, sequence_num, event_type,
                   payload, timestamp, duration_ms, parent_event_id, metadata
            FROM events
            WHERE execution_id = $1 AND event_type = $2
            ORDER BY sequence_num ASC
            "#,
        )
        .bind(execution_id)
        .bind(event_type.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_event).collect()
    }

    /// Get child events of a parent event
    #[instrument(skip(self))]
    pub async fn get_child_events(&self, parent_event_id: Uuid) -> Result<Vec<Event>> {
        let rows = sqlx::query(
            r#"
            SELECT id, execution_id, sequence_num, event_type,
                   payload, timestamp, duration_ms, parent_event_id, metadata
            FROM events
            WHERE parent_event_id = $1
            ORDER BY sequence_num ASC
            "#,
        )
        .bind(parent_event_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_event).collect()
    }

    /// Get the next sequence number for an execution
    #[instrument(skip(self))]
    pub async fn get_next_sequence_num(&self, execution_id: Uuid) -> Result<i32> {
        let row = sqlx::query(
            r#"
            SELECT COALESCE(MAX(sequence_num), 0) + 1 as next_num
            FROM events
            WHERE execution_id = $1
            "#,
        )
        .bind(execution_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.get::<i32, _>("next_num"))
    }

    /// Count events for an execution
    #[instrument(skip(self))]
    pub async fn count_events(&self, execution_id: Uuid) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM events
            WHERE execution_id = $1
            "#,
        )
        .bind(execution_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.get::<i64, _>("count"))
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    fn row_to_execution(row: sqlx::postgres::PgRow) -> Result<Execution> {
        let status_str: String = row.get("status");
        let status: ExecutionStatus = status_str
            .parse()
            .map_err(|e: String| Error::Serialization(e))?;

        Ok(Execution {
            id: row.get("id"),
            channel_type: row.get("channel_type"),
            channel_id: row.get("channel_id"),
            user_id: row.get("user_id"),
            thread_id: row.get("thread_id"),
            status,
            started_at: row.get("started_at"),
            completed_at: row.get("completed_at"),
            input_text: row.get("input_text"),
            output_text: row.get("output_text"),
            metadata: row.get("metadata"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    fn row_to_event(row: sqlx::postgres::PgRow) -> Result<Event> {
        let event_type_str: String = row.get("event_type");
        let event_type: EventType = event_type_str
            .parse()
            .map_err(|e: String| Error::Serialization(e))?;

        Ok(Event {
            id: row.get("id"),
            execution_id: row.get("execution_id"),
            sequence_num: row.get("sequence_num"),
            event_type,
            payload: row.get("payload"),
            timestamp: row.get("timestamp"),
            duration_ms: row.get("duration_ms"),
            parent_event_id: row.get("parent_event_id"),
            metadata: row.get("metadata"),
        })
    }
}

#[async_trait::async_trait]
impl EventStoreTrait for EventStore {
    async fn append(&self, event: Event) -> Result<()> {
        self.record_event(&event).await
    }

    async fn get_events(&self, execution_id: Uuid) -> Result<Vec<Event>> {
        self.get_execution_events(execution_id).await
    }

    fn name(&self) -> &str {
        "postgresql"
    }
}

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
        let next_seq = store.get_next_sequence_num(execution_id).await?;
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
        self.store.record_event(&event).await?;
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
        self.store.record_event(&event).await?;
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
        self.store.record_event(&event).await?;
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

/// Query options for listing executions
#[derive(Debug, Clone, Default)]
pub struct ExecutionQuery {
    /// Filter by channel type
    pub channel_type: Option<String>,
    /// Filter by channel ID
    pub channel_id: Option<String>,
    /// Filter by user ID
    pub user_id: Option<String>,
    /// Filter by status
    pub status: Option<ExecutionStatus>,
    /// Filter by time range (start)
    pub from_time: Option<DateTime<Utc>>,
    /// Filter by time range (end)
    pub to_time: Option<DateTime<Utc>>,
    /// Maximum results
    pub limit: i64,
    /// Offset for pagination
    pub offset: i64,
}

impl ExecutionQuery {
    /// Create a new query with default limits
    #[must_use]
    pub fn new() -> Self {
        Self {
            limit: 50,
            offset: 0,
            ..Default::default()
        }
    }

    /// Set the channel filter
    #[must_use]
    pub fn for_channel(mut self, channel_type: &str, channel_id: &str) -> Self {
        self.channel_type = Some(channel_type.to_string());
        self.channel_id = Some(channel_id.to_string());
        self
    }

    /// Set the user filter
    #[must_use]
    pub fn for_user(mut self, user_id: &str) -> Self {
        self.user_id = Some(user_id.to_string());
        self
    }

    /// Set the status filter
    #[must_use]
    pub fn with_status(mut self, status: ExecutionStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Set pagination
    #[must_use]
    pub fn paginate(mut self, limit: i64, offset: i64) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_query_builder() {
        let query = ExecutionQuery::new()
            .for_channel("telegram", "123")
            .for_user("user1")
            .with_status(ExecutionStatus::Completed)
            .paginate(10, 0);

        assert_eq!(query.channel_type, Some("telegram".to_string()));
        assert_eq!(query.channel_id, Some("123".to_string()));
        assert_eq!(query.user_id, Some("user1".to_string()));
        assert_eq!(query.status, Some(ExecutionStatus::Completed));
        assert_eq!(query.limit, 10);
        assert_eq!(query.offset, 0);
    }
}
