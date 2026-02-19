//! SQLite-based storage for skills and detected patterns.

use crate::analyzer::{DetectedPattern, PatternStatus};
use crate::error::{Error, Result};
use crate::skill::{
    Skill, SkillCategory, SkillMetadata, SkillOrigin, SkillStatus, SkillStep, SkillTrigger,
};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::Row;
use std::path::Path;
use tracing::{debug, info};
use uuid::Uuid;

/// Skill store database migrations.
pub mod migrations;
/// Skill pattern storage and management.
pub mod patterns;
/// Skill query and CRUD operations.
pub mod queries;
/// Skill execution statistics and metrics.
pub mod stats;

#[cfg(test)]
mod tests;

/// SQLite-based skill store
#[derive(Clone)]
pub struct SkillStore {
    pub(crate) pool: SqlitePool,
}

impl SkillStore {
    /// Create a new skill store with the given connection pool
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new skill store from a database path
    pub async fn from_path(db_path: &Path) -> Result<Self> {
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

        info!("SQLite skill store initialized at {}", db_path.display());
        Ok(store)
    }

    /// Create an in-memory store (for testing)
    pub async fn in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        let store = Self { pool };
        store.run_migrations().await?;

        debug!("In-memory SQLite skill store initialized");
        Ok(store)
    }

    /// Get a reference to the connection pool
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub(crate) fn row_to_skill(row: SqliteRow) -> Result<Skill> {
        let id_str: String = row.get("id");
        let category_str: String = row.get("category");
        let origin_str: String = row.get("origin");
        let status_str: String = row.get("status");
        let trigger_keywords_str: String = row.get("trigger_keywords");
        let trigger_regex_str: String = row.get("trigger_regex_patterns");
        let trigger_intents_str: String = row.get("trigger_intents");
        let steps_str: String = row.get("steps");
        let input_schema_str: Option<String> = row.get("input_schema");
        let last_used_str: Option<String> = row.get("last_used_at");
        let source_pattern_str: Option<String> = row.get("source_pattern_id");
        let created_at_str: String = row.get("created_at");
        let updated_at_str: String = row.get("updated_at");

        let id = Uuid::parse_str(&id_str)
            .map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
        let category: SkillCategory = category_str
            .parse()
            .map_err(|e: String| Error::Serialization(e))?;
        let origin: SkillOrigin = origin_str
            .parse()
            .map_err(|e: String| Error::Serialization(e))?;
        let status: SkillStatus = status_str
            .parse()
            .map_err(|e: String| Error::Serialization(e))?;

        let trigger_keywords: Vec<String> = serde_json::from_str(&trigger_keywords_str)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let trigger_regex: Vec<String> = serde_json::from_str(&trigger_regex_str)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let trigger_intents: Vec<String> = serde_json::from_str(&trigger_intents_str)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let steps: Vec<SkillStep> =
            serde_json::from_str(&steps_str).map_err(|e| Error::Serialization(e.to_string()))?;
        let input_schema = input_schema_str
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| Error::Serialization(e.to_string()))?;

        let last_used_at = last_used_str
            .map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))
            })
            .transpose()?;
        let source_pattern_id = source_pattern_str
            .map(|s| {
                Uuid::parse_str(&s).map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))
            })
            .transpose()?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))?
            .with_timezone(&Utc);

        Ok(Skill {
            id,
            name: row.get("name"),
            description: row.get("description"),
            category,
            origin,
            status,
            trigger: SkillTrigger {
                keywords: trigger_keywords,
                regex_patterns: trigger_regex,
                intents: trigger_intents,
                priority: row.get("trigger_priority"),
            },
            steps,
            input_schema,
            metadata: SkillMetadata {
                usage_count: row.get::<i64, _>("usage_count") as u64,
                success_rate: row.get("success_rate"),
                avg_duration_ms: row
                    .get::<Option<i64>, _>("avg_duration_ms")
                    .map(|d| d as u64),
                last_used_at,
                source_pattern_id,
            },
            created_at,
            updated_at,
        })
    }

    pub(crate) fn row_to_pattern(row: SqliteRow) -> Result<DetectedPattern> {
        let id_str: String = row.get("id");
        let tool_sequence_str: String = row.get("tool_sequence");
        let keywords_str: String = row.get("extracted_keywords");
        let samples_str: String = row.get("sample_inputs");
        let status_str: String = row.get("status");
        let converted_skill_str: Option<String> = row.get("converted_skill_id");
        let detected_at_str: String = row.get("detected_at");

        let id = Uuid::parse_str(&id_str)
            .map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
        let tool_sequence: Vec<String> = serde_json::from_str(&tool_sequence_str)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let extracted_keywords: Vec<String> =
            serde_json::from_str(&keywords_str).map_err(|e| Error::Serialization(e.to_string()))?;
        let sample_inputs: Vec<String> =
            serde_json::from_str(&samples_str).map_err(|e| Error::Serialization(e.to_string()))?;
        let status: PatternStatus = status_str
            .parse()
            .map_err(|e: String| Error::Serialization(e))?;
        let converted_skill_id = converted_skill_str
            .map(|s| {
                Uuid::parse_str(&s).map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))
            })
            .transpose()?;
        let detected_at = DateTime::parse_from_rfc3339(&detected_at_str)
            .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))?
            .with_timezone(&Utc);

        Ok(DetectedPattern {
            id,
            tool_sequence,
            occurrence_count: row.get::<i32, _>("occurrence_count") as u32,
            confidence_score: row.get("confidence_score"),
            extracted_keywords,
            sample_inputs,
            status,
            converted_skill_id,
            detected_at,
        })
    }
}
