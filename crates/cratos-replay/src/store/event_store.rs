//! EventStore - SQLite-based event storage

use super::helpers::row_to_execution;
use super::traits::EventStoreTrait;
use crate::error::{Error, Result};
use crate::event::{Event, EventType, Execution, ExecutionStatus};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;
use tracing::{debug, info, instrument};
use uuid::Uuid;

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
                session_id TEXT,
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
            CREATE INDEX IF NOT EXISTS idx_executions_session
            ON executions(session_id)
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
                id, channel_type, channel_id, user_id, session_id, thread_id,
                status, started_at, completed_at,
                input_text, output_text, metadata, created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14
            )
            "#,
        )
        .bind(execution.id.to_string())
        .bind(&execution.channel_type)
        .bind(&execution.channel_id)
        .bind(&execution.user_id)
        .bind(&execution.session_id)
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
            SELECT id, channel_type, channel_id, user_id, session_id, thread_id,
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

        row_to_execution(row)
    }

    /// Update execution status (typed)
    pub async fn update_execution_status_typed(
        &self,
        id: Uuid,
        status: ExecutionStatus,
        output_text: Option<&str>,
    ) -> Result<()> {
        self.update_execution_status_str(id, status.as_str(), output_text)
            .await
    }

    /// Update execution status (string-based, used by trait)
    #[instrument(skip(self))]
    pub async fn update_execution_status_str(
        &self,
        id: Uuid,
        status: &str,
        output_text: Option<&str>,
    ) -> Result<()> {
        let is_terminal = matches!(status, "completed" | "failed" | "cancelled");
        let completed_at = if is_terminal {
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
        .bind(status)
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
            SELECT id, channel_type, channel_id, user_id, session_id, thread_id,
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

        rows.into_iter().map(row_to_execution).collect()
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
            SELECT id, channel_type, channel_id, user_id, session_id, thread_id,
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

        rows.into_iter().map(row_to_execution).collect()
    }

    /// List executions for a session
    #[instrument(skip(self))]
    pub async fn list_executions_by_session(
        &self,
        session_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Execution>> {
        let rows = sqlx::query(
            r#"
            SELECT id, channel_type, channel_id, user_id, session_id, thread_id,
                   status, started_at, completed_at,
                   input_text, output_text, metadata, created_at, updated_at
            FROM executions
            WHERE session_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2 OFFSET ?3
            "#,
        )
        .bind(session_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(row_to_execution).collect()
    }

    /// List recent executions
    #[instrument(skip(self))]
    pub async fn list_recent_executions(&self, limit: i64) -> Result<Vec<Execution>> {
        let rows = sqlx::query(
            r#"
            SELECT id, channel_type, channel_id, user_id, session_id, thread_id,
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

        rows.into_iter().map(row_to_execution).collect()
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
    // Event operations (wrappers for backward compatibility)
    // =========================================================================

    /// Record a new event
    #[instrument(skip(self, event), fields(event_id = %event.id, execution_id = %event.execution_id))]
    pub async fn record_event(&self, event: &Event) -> Result<()> {
        super::events::record_event(self, event).await
    }

    /// Get an event by ID
    #[instrument(skip(self))]
    pub async fn get_event(&self, id: Uuid) -> Result<Event> {
        super::events::get_event(self, id).await
    }

    /// Get all events for an execution
    #[instrument(skip(self))]
    pub async fn get_execution_events(&self, execution_id: Uuid) -> Result<Vec<Event>> {
        super::events::get_execution_events(self, execution_id).await
    }

    /// Get events by type for an execution
    #[instrument(skip(self))]
    pub async fn get_events_by_type(
        &self,
        execution_id: Uuid,
        event_type: EventType,
    ) -> Result<Vec<Event>> {
        super::events::get_events_by_type(self, execution_id, event_type).await
    }

    /// Get child events of a parent event
    #[instrument(skip(self))]
    pub async fn get_child_events(&self, parent_event_id: Uuid) -> Result<Vec<Event>> {
        super::events::get_child_events(self, parent_event_id).await
    }

    /// Get the next sequence number for an execution
    #[instrument(skip(self))]
    pub async fn get_next_sequence_num(&self, execution_id: Uuid) -> Result<i32> {
        super::events::get_next_sequence_num(self, execution_id).await
    }

    /// Count events for an execution
    #[instrument(skip(self))]
    pub async fn count_events(&self, execution_id: Uuid) -> Result<i64> {
        super::events::count_events(self, execution_id).await
    }
}

#[async_trait::async_trait]
impl EventStoreTrait for EventStore {
    async fn create_execution(&self, execution: &Execution) -> Result<()> {
        EventStore::create_execution(self, execution).await
    }

    async fn append(&self, event: Event) -> Result<()> {
        super::events::record_event(self, &event).await
    }

    async fn get_events(&self, execution_id: Uuid) -> Result<Vec<Event>> {
        super::events::get_execution_events(self, execution_id).await
    }

    async fn update_execution_status(
        &self,
        id: Uuid,
        status: &str,
        output_text: Option<&str>,
    ) -> Result<()> {
        self.update_execution_status_str(id, status, output_text)
            .await
    }

    fn name(&self) -> &str {
        "sqlite"
    }
}
