//! Event operations for EventStore

use super::helpers::row_to_event;
use super::event_store::EventStore;
use crate::error::{Error, Result};
use crate::event::{Event, EventType};
use sqlx::Row;
use tracing::{debug, instrument};
use uuid::Uuid;

/// Record a new event
#[instrument(skip(store, event), fields(event_id = %event.id, execution_id = %event.execution_id))]
pub async fn record_event(store: &EventStore, event: &Event) -> Result<()> {
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
    .execute(store.pool())
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    debug!(
        "Recorded event {} for execution {}",
        event.id, event.execution_id
    );
    Ok(())
}

/// Get an event by ID
#[instrument(skip(store))]
pub async fn get_event(store: &EventStore, id: Uuid) -> Result<Event> {
    let row = sqlx::query(
        r#"
        SELECT id, execution_id, sequence_num, event_type,
               payload, timestamp, duration_ms, parent_event_id, metadata
        FROM events
        WHERE id = ?1
        "#,
    )
    .bind(id.to_string())
    .fetch_optional(store.pool())
    .await
    .map_err(|e| Error::Database(e.to_string()))?
    .ok_or_else(|| Error::NotFound(id.to_string()))?;

    row_to_event(row)
}

/// Get all events for an execution
#[instrument(skip(store))]
pub async fn get_execution_events(store: &EventStore, execution_id: Uuid) -> Result<Vec<Event>> {
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
    .fetch_all(store.pool())
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    rows.into_iter().map(row_to_event).collect()
}

/// Get events by type for an execution
#[instrument(skip(store))]
pub async fn get_events_by_type(
    store: &EventStore,
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
    .fetch_all(store.pool())
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    rows.into_iter().map(row_to_event).collect()
}

/// Get child events of a parent event
#[instrument(skip(store))]
pub async fn get_child_events(store: &EventStore, parent_event_id: Uuid) -> Result<Vec<Event>> {
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
    .fetch_all(store.pool())
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    rows.into_iter().map(row_to_event).collect()
}

/// Get the next sequence number for an execution
#[instrument(skip(store))]
pub async fn get_next_sequence_num(store: &EventStore, execution_id: Uuid) -> Result<i32> {
    let row = sqlx::query(
        r#"
        SELECT COALESCE(MAX(sequence_num), 0) + 1 as next_num
        FROM events
        WHERE execution_id = ?1
        "#,
    )
    .bind(execution_id.to_string())
    .fetch_one(store.pool())
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(row.get::<i32, _>("next_num"))
}

/// Count events for an execution
#[instrument(skip(store))]
pub async fn count_events(store: &EventStore, execution_id: Uuid) -> Result<i64> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) as count
        FROM events
        WHERE execution_id = ?1
        "#,
    )
    .bind(execution_id.to_string())
    .fetch_one(store.pool())
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    Ok(row.get::<i64, _>("count"))
}
