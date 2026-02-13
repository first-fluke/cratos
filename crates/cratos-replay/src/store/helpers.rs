//! Helper functions for store module

use crate::error::Error;
use crate::event::{Event, EventType, Execution, ExecutionStatus};
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

/// Convert a SQLite row to an Execution
pub(crate) fn row_to_execution(row: SqliteRow) -> Result<Execution, Error> {
    let id_str: String = row.get("id");
    let status_str: String = row.get("status");
    let started_at_str: String = row.get("started_at");
    let completed_at_str: Option<String> = row.get("completed_at");
    let metadata_str: String = row.get("metadata");
    let created_at_str: String = row.get("created_at");
    let updated_at_str: String = row.get("updated_at");

    let id =
        Uuid::parse_str(&id_str).map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
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

/// Convert a SQLite row to an Event
pub(crate) fn row_to_event(row: SqliteRow) -> Result<Event, Error> {
    let id_str: String = row.get("id");
    let execution_id_str: String = row.get("execution_id");
    let event_type_str: String = row.get("event_type");
    let payload_str: String = row.get("payload");
    let timestamp_str: String = row.get("timestamp");
    let parent_event_id_str: Option<String> = row.get("parent_event_id");
    let metadata_str: String = row.get("metadata");

    let id =
        Uuid::parse_str(&id_str).map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
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

/// Get the default data directory for Cratos
pub fn default_data_dir() -> std::path::PathBuf {
    // Always use ~/.cratos/ for consistency across all components
    // (config, chronicles, memory, vectors all live here)
    dirs::home_dir()
        .map(|p| p.join(".cratos"))
        .unwrap_or_else(|| std::path::PathBuf::from(".cratos"))
}

/// Get the default database path
pub fn default_db_path() -> std::path::PathBuf {
    default_data_dir().join("cratos.db")
}
