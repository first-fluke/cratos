//! Store - Event persistence using SQLite
//!
//! This module provides the storage layer for executions and events.
//! It uses sqlx for async SQLite access (embedded, no Docker required).

use crate::error::{Error, Result};
use crate::event::{Event, EventType, Execution, ExecutionStatus};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::Row;
use std::path::Path;
use tracing::{debug, info, instrument};
use uuid::Uuid;

/// Trait for event storage backends
///
/// This trait allows different storage implementations (SQLite, in-memory, etc.)
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

/// Event store for persisting executions and events to SQLite
#[derive(Clone)]
pub struct EventStore {
    pool: SqlitePool,
}

impl EventStore {
    /// Create a new event store with the given connection pool
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new event store from a database path
    ///
    /// This will create the database file if it doesn't exist and run migrations.
    pub async fn from_path(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Database(format!("failed to create directory: {e}")))?;
        }

        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        let store = Self { pool };
        store.run_migrations().await?;

        info!("SQLite event store initialized at {}", db_path.display());
        Ok(store)
    }

    /// Create a new in-memory event store (for testing)
    pub async fn in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        let store = Self { pool };
        store.run_migrations().await?;

        debug!("In-memory SQLite event store initialized");
        Ok(store)
    }

    /// Run database migrations
    async fn run_migrations(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS executions (
                id TEXT PRIMARY KEY,
                channel_type TEXT NOT NULL,
                channel_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                thread_id TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                started_at TEXT NOT NULL,
                completed_at TEXT,
                input_text TEXT NOT NULL,
                output_text TEXT,
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                execution_id TEXT NOT NULL,
                sequence_num INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL DEFAULT '{}',
                timestamp TEXT NOT NULL,
                duration_ms INTEGER,
                parent_event_id TEXT,
                metadata TEXT NOT NULL DEFAULT '{}',
                FOREIGN KEY (execution_id) REFERENCES executions(id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Create indexes
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_executions_channel
            ON executions(channel_type, channel_id)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_executions_user
            ON executions(user_id)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_executions_created
            ON executions(created_at DESC)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_events_execution
            ON events(execution_id, sequence_num)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_events_type
            ON events(execution_id, event_type)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!("Database migrations completed");
        Ok(())
    }

    /// Get a reference to the underlying connection pool
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
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
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8,
                ?9, ?10, ?11, ?12, ?13
            )
            "#,
        )
        .bind(execution.id.to_string())
        .bind(&execution.channel_type)
        .bind(&execution.channel_id)
        .bind(&execution.user_id)
        .bind(&execution.thread_id)
        .bind(execution.status.as_str())
        .bind(execution.started_at.to_rfc3339())
        .bind(execution.completed_at.map(|t| t.to_rfc3339()))
        .bind(&execution.input_text)
        .bind(&execution.output_text)
        .bind(execution.metadata.to_string())
        .bind(execution.created_at.to_rfc3339())
        .bind(execution.updated_at.to_rfc3339())
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
            WHERE id = ?1
            "#,
        )
        .bind(id.to_string())
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
            Some(Utc::now().to_rfc3339())
        } else {
            None
        };

        sqlx::query(
            r#"
            UPDATE executions
            SET status = ?2, output_text = COALESCE(?3, output_text),
                completed_at = COALESCE(?4, completed_at), updated_at = ?5
            WHERE id = ?1
            "#,
        )
        .bind(id.to_string())
        .bind(status.as_str())
        .bind(output_text)
        .bind(completed_at)
        .bind(Utc::now().to_rfc3339())
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
            WHERE channel_type = ?1 AND channel_id = ?2
            ORDER BY created_at DESC
            LIMIT ?3 OFFSET ?4
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
            WHERE user_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2 OFFSET ?3
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
            LIMIT ?1
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
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9
            )
            "#,
        )
        .bind(event.id.to_string())
        .bind(event.execution_id.to_string())
        .bind(event.sequence_num)
        .bind(event.event_type.as_str())
        .bind(event.payload.to_string())
        .bind(event.timestamp.to_rfc3339())
        .bind(event.duration_ms)
        .bind(event.parent_event_id.map(|id| id.to_string()))
        .bind(event.metadata.to_string())
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
            WHERE id = ?1
            "#,
        )
        .bind(id.to_string())
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
            WHERE execution_id = ?1
            ORDER BY sequence_num ASC
            "#,
        )
        .bind(execution_id.to_string())
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
            WHERE execution_id = ?1 AND event_type = ?2
            ORDER BY sequence_num ASC
            "#,
        )
        .bind(execution_id.to_string())
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
            WHERE parent_event_id = ?1
            ORDER BY sequence_num ASC
            "#,
        )
        .bind(parent_event_id.to_string())
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
            WHERE execution_id = ?1
            "#,
        )
        .bind(execution_id.to_string())
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
            WHERE execution_id = ?1
            "#,
        )
        .bind(execution_id.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.get::<i64, _>("count"))
    }

    /// Delete old executions (for cleanup/retention)
    #[instrument(skip(self))]
    pub async fn delete_old_executions(&self, before: DateTime<Utc>) -> Result<u64> {
        // First delete events for old executions
        sqlx::query(
            r#"
            DELETE FROM events
            WHERE execution_id IN (
                SELECT id FROM executions WHERE created_at < ?1
            )
            "#,
        )
        .bind(before.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Then delete the executions
        let result = sqlx::query(
            r#"
            DELETE FROM executions WHERE created_at < ?1
            "#,
        )
        .bind(before.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(result.rows_affected())
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    fn row_to_execution(row: SqliteRow) -> Result<Execution> {
        let id_str: String = row.get("id");
        let status_str: String = row.get("status");
        let started_at_str: String = row.get("started_at");
        let completed_at_str: Option<String> = row.get("completed_at");
        let metadata_str: String = row.get("metadata");
        let created_at_str: String = row.get("created_at");
        let updated_at_str: String = row.get("updated_at");

        let id = Uuid::parse_str(&id_str)
            .map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
        let status: ExecutionStatus = status_str
            .parse()
            .map_err(|e: String| Error::Serialization(e))?;
        let started_at = DateTime::parse_from_rfc3339(&started_at_str)
            .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))?
            .with_timezone(&Utc);
        let completed_at = completed_at_str
            .map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))
            })
            .transpose()?;
        let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
            .map_err(|e| Error::Serialization(format!("invalid json: {e}")))?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))?
            .with_timezone(&Utc);

        Ok(Execution {
            id,
            channel_type: row.get("channel_type"),
            channel_id: row.get("channel_id"),
            user_id: row.get("user_id"),
            thread_id: row.get("thread_id"),
            status,
            started_at,
            completed_at,
            input_text: row.get("input_text"),
            output_text: row.get("output_text"),
            metadata,
            created_at,
            updated_at,
        })
    }

    fn row_to_event(row: SqliteRow) -> Result<Event> {
        let id_str: String = row.get("id");
        let execution_id_str: String = row.get("execution_id");
        let event_type_str: String = row.get("event_type");
        let payload_str: String = row.get("payload");
        let timestamp_str: String = row.get("timestamp");
        let parent_event_id_str: Option<String> = row.get("parent_event_id");
        let metadata_str: String = row.get("metadata");

        let id = Uuid::parse_str(&id_str)
            .map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
        let execution_id = Uuid::parse_str(&execution_id_str)
            .map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
        let event_type: EventType = event_type_str
            .parse()
            .map_err(|e: String| Error::Serialization(e))?;
        let payload: serde_json::Value = serde_json::from_str(&payload_str)
            .map_err(|e| Error::Serialization(format!("invalid json: {e}")))?;
        let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
            .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))?
            .with_timezone(&Utc);
        let parent_event_id = parent_event_id_str
            .map(|s| {
                Uuid::parse_str(&s).map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))
            })
            .transpose()?;
        let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
            .map_err(|e| Error::Serialization(format!("invalid json: {e}")))?;

        Ok(Event {
            id,
            execution_id,
            sequence_num: row.get("sequence_num"),
            event_type,
            payload,
            timestamp,
            duration_ms: row.get("duration_ms"),
            parent_event_id,
            metadata,
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
        "sqlite"
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

/// Get the default data directory for Cratos
pub fn default_data_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .map(|p| p.join(".cratos"))
        .unwrap_or_else(|| std::path::PathBuf::from(".cratos"))
}

/// Get the default database path
pub fn default_db_path() -> std::path::PathBuf {
    default_data_dir().join("cratos.db")
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

    #[test]
    fn test_default_data_dir() {
        let dir = default_data_dir();
        assert!(dir.to_string_lossy().contains("cratos"));
    }

    #[tokio::test]
    async fn test_in_memory_store() {
        let store = EventStore::in_memory().await.unwrap();
        assert_eq!(store.name(), "sqlite");

        // Create an execution
        let execution = Execution::new("telegram", "12345", "user1", "Hello, world!");
        store.create_execution(&execution).await.unwrap();

        // Retrieve it
        let retrieved = store.get_execution(execution.id).await.unwrap();
        assert_eq!(retrieved.id, execution.id);
        assert_eq!(retrieved.input_text, "Hello, world!");
    }

    #[tokio::test]
    async fn test_event_recording() {
        let store = EventStore::in_memory().await.unwrap();

        // Create an execution
        let execution = Execution::new("telegram", "12345", "user1", "Hello");
        store.create_execution(&execution).await.unwrap();

        // Create a recorder
        let recorder = EventRecorder::new(store.clone(), execution.id);

        // Record some events
        recorder.record_user_input("Hello").await.unwrap();
        recorder
            .record_llm_request("openai", "gpt-4", 1, &[])
            .await
            .unwrap();
        recorder
            .record_llm_response("openai", "gpt-4", "Hi!", false, None, 100)
            .await
            .unwrap();

        // Verify
        let events = store.get_execution_events(execution.id).await.unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, EventType::UserInput);
        assert_eq!(events[1].event_type, EventType::LlmRequest);
        assert_eq!(events[2].event_type, EventType::LlmResponse);
    }
}
