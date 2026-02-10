//! SQLite-based storage for skills and detected patterns.
//!
//! This module provides persistent storage using SQLite (embedded, no Docker required).
//! Following the same pattern as cratos-replay for consistency.
//!
//! # Overview
//!
//! The [`SkillStore`] manages three types of data:
//!
//! | Table | Description |
//! |-------|-------------|
//! | `skills` | Skill definitions with triggers, steps, and metadata |
//! | `detected_patterns` | Patterns waiting for conversion or rejection |
//! | `skill_executions` | Execution history for analytics |
//!
//! # Storage Location
//!
//! Default path: `~/.cratos/skills.db`
//!
//! Use [`crate::default_skill_db_path()`] to get the default path.
//!
//! # Example
//!
//! ```ignore
//! use cratos_skills::{SkillStore, default_skill_db_path};
//!
//! // Create or open store
//! let store = SkillStore::from_path(&default_skill_db_path()).await?;
//!
//! // For testing, use in-memory store
//! let test_store = SkillStore::in_memory().await?;
//!
//! // Skill CRUD operations
//! store.save_skill(&skill).await?;
//! let skill = store.get_skill(skill_id).await?;
//! let skill = store.get_skill_by_name("my_skill").await?;
//! store.delete_skill(skill_id).await?;
//!
//! // List skills
//! let all = store.list_skills().await?;
//! let active = store.list_active_skills().await?;
//! let workflows = store.list_skills_by_category(SkillCategory::Workflow).await?;
//!
//! // Pattern management
//! store.save_pattern(&pattern).await?;
//! store.mark_pattern_converted(pattern_id, skill_id).await?;
//! store.mark_pattern_rejected(pattern_id).await?;
//! let pending = store.list_detected_patterns().await?;
//!
//! // Execution tracking
//! store.record_skill_execution(skill_id, None, true, Some(100), &[]).await?;
//! let (total, successes) = store.get_skill_execution_count(skill_id).await?;
//! ```
//!
//! # Schema
//!
//! The store automatically creates tables on first use. See the module source
//! for the complete schema definition.

use crate::analyzer::{DetectedPattern, PatternStatus};
use crate::error::{Error, Result};
use crate::skill::{
    Skill, SkillCategory, SkillMetadata, SkillOrigin, SkillStatus, SkillStep, SkillTrigger,
};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::Row;
use std::path::Path;
use tracing::{debug, info, instrument};
use uuid::Uuid;

/// SQLite-based skill store
#[derive(Clone)]
pub struct SkillStore {
    pool: SqlitePool,
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

    /// Run database migrations
    async fn run_migrations(&self) -> Result<()> {
        // Skills table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'custom',
                origin TEXT NOT NULL DEFAULT 'user_defined',
                status TEXT NOT NULL DEFAULT 'draft',

                trigger_keywords TEXT NOT NULL DEFAULT '[]',
                trigger_regex_patterns TEXT NOT NULL DEFAULT '[]',
                trigger_intents TEXT NOT NULL DEFAULT '[]',
                trigger_priority INTEGER NOT NULL DEFAULT 0,

                steps TEXT NOT NULL DEFAULT '[]',
                input_schema TEXT,

                usage_count INTEGER NOT NULL DEFAULT 0,
                success_rate REAL NOT NULL DEFAULT 1.0,
                avg_duration_ms INTEGER,
                last_used_at TEXT,
                source_pattern_id TEXT,

                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Detected patterns table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS detected_patterns (
                id TEXT PRIMARY KEY,
                tool_sequence TEXT NOT NULL,
                occurrence_count INTEGER NOT NULL,
                confidence_score REAL NOT NULL,
                extracted_keywords TEXT NOT NULL DEFAULT '[]',
                sample_inputs TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'detected',
                converted_skill_id TEXT,
                detected_at TEXT NOT NULL,

                FOREIGN KEY (converted_skill_id) REFERENCES skills(id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Skill executions table (for tracking usage)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS skill_executions (
                id TEXT PRIMARY KEY,
                skill_id TEXT NOT NULL,
                execution_id TEXT,
                success INTEGER NOT NULL,
                duration_ms INTEGER,
                step_results TEXT NOT NULL DEFAULT '[]',
                started_at TEXT NOT NULL,

                FOREIGN KEY (skill_id) REFERENCES skills(id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Create indexes
        sqlx::query(r#"CREATE INDEX IF NOT EXISTS idx_skills_status ON skills(status)"#)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(r#"CREATE INDEX IF NOT EXISTS idx_skills_category ON skills(category)"#)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_patterns_status ON detected_patterns(status)"#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_skill_executions_skill ON skill_executions(skill_id)"#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!("Skill store migrations completed");
        Ok(())
    }

    /// Get a reference to the connection pool
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // =========================================================================
    // Skill operations
    // =========================================================================

    /// Save a skill (insert or update)
    #[instrument(skip(self, skill), fields(skill_id = %skill.id, skill_name = %skill.name))]
    pub async fn save_skill(&self, skill: &Skill) -> Result<()> {
        let trigger_keywords = serde_json::to_string(&skill.trigger.keywords)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let trigger_regex = serde_json::to_string(&skill.trigger.regex_patterns)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let trigger_intents = serde_json::to_string(&skill.trigger.intents)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let steps =
            serde_json::to_string(&skill.steps).map_err(|e| Error::Serialization(e.to_string()))?;
        let input_schema = skill.input_schema.as_ref().map(|s| s.to_string());

        sqlx::query(
            r#"
            INSERT INTO skills (
                id, name, description, category, origin, status,
                trigger_keywords, trigger_regex_patterns, trigger_intents, trigger_priority,
                steps, input_schema,
                usage_count, success_rate, avg_duration_ms, last_used_at, source_pattern_id,
                created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10,
                ?11, ?12,
                ?13, ?14, ?15, ?16, ?17,
                ?18, ?19
            )
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                category = excluded.category,
                origin = excluded.origin,
                status = excluded.status,
                trigger_keywords = excluded.trigger_keywords,
                trigger_regex_patterns = excluded.trigger_regex_patterns,
                trigger_intents = excluded.trigger_intents,
                trigger_priority = excluded.trigger_priority,
                steps = excluded.steps,
                input_schema = excluded.input_schema,
                usage_count = excluded.usage_count,
                success_rate = excluded.success_rate,
                avg_duration_ms = excluded.avg_duration_ms,
                last_used_at = excluded.last_used_at,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(skill.id.to_string())
        .bind(&skill.name)
        .bind(&skill.description)
        .bind(skill.category.as_str())
        .bind(skill.origin.as_str())
        .bind(skill.status.as_str())
        .bind(&trigger_keywords)
        .bind(&trigger_regex)
        .bind(&trigger_intents)
        .bind(skill.trigger.priority)
        .bind(&steps)
        .bind(&input_schema)
        .bind(skill.metadata.usage_count as i64)
        .bind(skill.metadata.success_rate)
        .bind(skill.metadata.avg_duration_ms.map(|d| d as i64))
        .bind(skill.metadata.last_used_at.map(|t| t.to_rfc3339()))
        .bind(skill.metadata.source_pattern_id.map(|id| id.to_string()))
        .bind(skill.created_at.to_rfc3339())
        .bind(skill.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!("Saved skill: {} ({})", skill.name, skill.id);
        Ok(())
    }

    /// Get a skill by ID
    #[instrument(skip(self))]
    pub async fn get_skill(&self, id: Uuid) -> Result<Skill> {
        let row = sqlx::query(
            r#"
            SELECT * FROM skills WHERE id = ?1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::SkillNotFound(id.to_string()))?;

        Self::row_to_skill(row)
    }

    /// Get a skill by name
    #[instrument(skip(self))]
    pub async fn get_skill_by_name(&self, name: &str) -> Result<Option<Skill>> {
        let row = sqlx::query(
            r#"
            SELECT * FROM skills WHERE name = ?1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        match row {
            Some(r) => Ok(Some(Self::row_to_skill(r)?)),
            None => Ok(None),
        }
    }

    /// List all skills
    #[instrument(skip(self))]
    pub async fn list_skills(&self) -> Result<Vec<Skill>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM skills ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_skill).collect()
    }

    /// List active skills
    #[instrument(skip(self))]
    pub async fn list_active_skills(&self) -> Result<Vec<Skill>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM skills WHERE status = 'active' ORDER BY trigger_priority DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_skill).collect()
    }

    /// List skills by category
    #[instrument(skip(self))]
    pub async fn list_skills_by_category(&self, category: SkillCategory) -> Result<Vec<Skill>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM skills WHERE category = ?1 ORDER BY updated_at DESC
            "#,
        )
        .bind(category.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_skill).collect()
    }

    /// Delete a skill
    #[instrument(skip(self))]
    pub async fn delete_skill(&self, id: Uuid) -> Result<()> {
        // First delete related skill executions
        sqlx::query(r#"DELETE FROM skill_executions WHERE skill_id = ?1"#)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        // Then delete the skill
        let result = sqlx::query(r#"DELETE FROM skills WHERE id = ?1"#)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(Error::SkillNotFound(id.to_string()));
        }

        debug!("Deleted skill: {}", id);
        Ok(())
    }

    // =========================================================================
    // Pattern operations
    // =========================================================================

    /// Save a detected pattern
    #[instrument(skip(self, pattern), fields(pattern_id = %pattern.id))]
    pub async fn save_pattern(&self, pattern: &DetectedPattern) -> Result<()> {
        let tool_sequence = serde_json::to_string(&pattern.tool_sequence)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let keywords = serde_json::to_string(&pattern.extracted_keywords)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let samples = serde_json::to_string(&pattern.sample_inputs)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO detected_patterns (
                id, tool_sequence, occurrence_count, confidence_score,
                extracted_keywords, sample_inputs, status, converted_skill_id, detected_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9
            )
            ON CONFLICT(id) DO UPDATE SET
                occurrence_count = excluded.occurrence_count,
                confidence_score = excluded.confidence_score,
                extracted_keywords = excluded.extracted_keywords,
                sample_inputs = excluded.sample_inputs,
                status = excluded.status,
                converted_skill_id = excluded.converted_skill_id
            "#,
        )
        .bind(pattern.id.to_string())
        .bind(&tool_sequence)
        .bind(pattern.occurrence_count as i32)
        .bind(pattern.confidence_score)
        .bind(&keywords)
        .bind(&samples)
        .bind(pattern.status.as_str())
        .bind(pattern.converted_skill_id.map(|id| id.to_string()))
        .bind(pattern.detected_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!("Saved pattern: {}", pattern.id);
        Ok(())
    }

    /// Get a pattern by ID
    #[instrument(skip(self))]
    pub async fn get_pattern(&self, id: Uuid) -> Result<DetectedPattern> {
        let row = sqlx::query(r#"SELECT * FROM detected_patterns WHERE id = ?1"#)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .ok_or_else(|| Error::PatternNotFound(id.to_string()))?;

        Self::row_to_pattern(row)
    }

    /// List patterns by status
    #[instrument(skip(self))]
    pub async fn list_patterns_by_status(
        &self,
        status: PatternStatus,
    ) -> Result<Vec<DetectedPattern>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM detected_patterns WHERE status = ?1
            ORDER BY confidence_score DESC, occurrence_count DESC
            "#,
        )
        .bind(status.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_pattern).collect()
    }

    /// List all detected (unconverted) patterns
    #[instrument(skip(self))]
    pub async fn list_detected_patterns(&self) -> Result<Vec<DetectedPattern>> {
        self.list_patterns_by_status(PatternStatus::Detected).await
    }

    /// Mark a pattern as converted
    #[instrument(skip(self))]
    pub async fn mark_pattern_converted(&self, pattern_id: Uuid, skill_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE detected_patterns
            SET status = 'converted', converted_skill_id = ?2
            WHERE id = ?1
            "#,
        )
        .bind(pattern_id.to_string())
        .bind(skill_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!(
            "Marked pattern {} as converted to skill {}",
            pattern_id, skill_id
        );
        Ok(())
    }

    /// Mark a pattern as rejected
    #[instrument(skip(self))]
    pub async fn mark_pattern_rejected(&self, pattern_id: Uuid) -> Result<()> {
        sqlx::query(r#"UPDATE detected_patterns SET status = 'rejected' WHERE id = ?1"#)
            .bind(pattern_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        debug!("Marked pattern {} as rejected", pattern_id);
        Ok(())
    }

    // =========================================================================
    // Skill execution tracking
    // =========================================================================

    /// Record a skill execution
    #[instrument(skip(self))]
    pub async fn record_skill_execution(
        &self,
        skill_id: Uuid,
        execution_id: Option<Uuid>,
        success: bool,
        duration_ms: Option<u64>,
        step_results: &[serde_json::Value],
    ) -> Result<()> {
        let step_results_json =
            serde_json::to_string(step_results).map_err(|e| Error::Serialization(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO skill_executions (
                id, skill_id, execution_id, success, duration_ms, step_results, started_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7
            )
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(skill_id.to_string())
        .bind(execution_id.map(|id| id.to_string()))
        .bind(success)
        .bind(duration_ms.map(|d| d as i64))
        .bind(&step_results_json)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!("Recorded skill execution for skill {}", skill_id);
        Ok(())
    }

    /// Update skill metrics (usage_count, success_rate, avg_duration_ms, last_used_at) in DB.
    ///
    /// Called after each skill execution to persist the in-memory metric updates.
    #[instrument(skip(self))]
    pub async fn update_skill_metrics(
        &self,
        skill_id: Uuid,
        usage_count: u64,
        success_rate: f64,
        avg_duration_ms: Option<u64>,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE skills
            SET usage_count = ?2,
                success_rate = ?3,
                avg_duration_ms = ?4,
                last_used_at = ?5,
                status = ?6,
                updated_at = ?5
            WHERE id = ?1
            "#,
        )
        .bind(skill_id.to_string())
        .bind(usage_count as i64)
        .bind(success_rate)
        .bind(avg_duration_ms.map(|d| d as i64))
        .bind(Utc::now().to_rfc3339())
        .bind(status)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!(
            "Updated skill {} metrics: usage={}, rate={:.2}, status={}",
            skill_id, usage_count, success_rate, status
        );
        Ok(())
    }

    /// Get execution count for a skill
    #[instrument(skip(self))]
    pub async fn get_skill_execution_count(&self, skill_id: Uuid) -> Result<(u64, u64)> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total,
                SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as successes
            FROM skill_executions
            WHERE skill_id = ?1
            "#,
        )
        .bind(skill_id.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        let total: i64 = row.get("total");
        let successes: i64 = row.get("successes");

        Ok((total as u64, successes as u64))
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    fn row_to_skill(row: SqliteRow) -> Result<Skill> {
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

    fn row_to_pattern(row: SqliteRow) -> Result<DetectedPattern> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::SkillStep;

    async fn create_test_store() -> SkillStore {
        SkillStore::in_memory().await.unwrap()
    }

    fn create_test_skill() -> Skill {
        Skill::new("test_skill", "A test skill", SkillCategory::Custom)
            .with_trigger(SkillTrigger::with_keywords(vec!["test".to_string()]))
            .with_step(SkillStep::new(
                1,
                "file_read",
                serde_json::json!({"path": "{{path}}"}),
            ))
    }

    fn create_test_pattern() -> DetectedPattern {
        DetectedPattern {
            id: Uuid::new_v4(),
            tool_sequence: vec!["file_read".to_string(), "git_commit".to_string()],
            occurrence_count: 5,
            confidence_score: 0.8,
            extracted_keywords: vec!["read".to_string()],
            sample_inputs: vec!["test input".to_string()],
            status: PatternStatus::Detected,
            converted_skill_id: None,
            detected_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_save_and_get_skill() {
        let store = create_test_store().await;
        let skill = create_test_skill();

        store.save_skill(&skill).await.unwrap();

        let retrieved = store.get_skill(skill.id).await.unwrap();
        assert_eq!(retrieved.name, skill.name);
        assert_eq!(retrieved.steps.len(), 1);
    }

    #[tokio::test]
    async fn test_get_skill_by_name() {
        let store = create_test_store().await;
        let skill = create_test_skill();

        store.save_skill(&skill).await.unwrap();

        let retrieved = store.get_skill_by_name("test_skill").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, skill.id);
    }

    #[tokio::test]
    async fn test_list_active_skills() {
        let store = create_test_store().await;

        let mut skill1 = create_test_skill();
        skill1.name = "skill1".to_string();
        skill1.activate();
        store.save_skill(&skill1).await.unwrap();

        let mut skill2 = create_test_skill();
        skill2.name = "skill2".to_string();
        skill2.id = Uuid::new_v4();
        // skill2 is draft (default)
        store.save_skill(&skill2).await.unwrap();

        let active = store.list_active_skills().await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "skill1");
    }

    #[tokio::test]
    async fn test_delete_skill() {
        let store = create_test_store().await;
        let skill = create_test_skill();

        store.save_skill(&skill).await.unwrap();
        assert!(store.get_skill(skill.id).await.is_ok());

        store.delete_skill(skill.id).await.unwrap();
        assert!(store.get_skill(skill.id).await.is_err());
    }

    #[tokio::test]
    async fn test_save_and_get_pattern() {
        let store = create_test_store().await;
        let pattern = create_test_pattern();

        store.save_pattern(&pattern).await.unwrap();

        let retrieved = store.get_pattern(pattern.id).await.unwrap();
        assert_eq!(retrieved.tool_sequence, pattern.tool_sequence);
        assert_eq!(retrieved.occurrence_count, pattern.occurrence_count);
    }

    #[tokio::test]
    async fn test_list_detected_patterns() {
        let store = create_test_store().await;

        let pattern1 = create_test_pattern();
        store.save_pattern(&pattern1).await.unwrap();

        let mut pattern2 = create_test_pattern();
        pattern2.id = Uuid::new_v4();
        pattern2.status = PatternStatus::Converted;
        store.save_pattern(&pattern2).await.unwrap();

        let detected = store.list_detected_patterns().await.unwrap();
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].id, pattern1.id);
    }

    #[tokio::test]
    async fn test_mark_pattern_converted() {
        let store = create_test_store().await;
        let pattern = create_test_pattern();

        // Create a skill first to satisfy foreign key constraint
        let skill = create_test_skill();
        store.save_skill(&skill).await.unwrap();

        store.save_pattern(&pattern).await.unwrap();
        store
            .mark_pattern_converted(pattern.id, skill.id)
            .await
            .unwrap();

        let updated = store.get_pattern(pattern.id).await.unwrap();
        assert_eq!(updated.status, PatternStatus::Converted);
        assert_eq!(updated.converted_skill_id, Some(skill.id));
    }

    #[tokio::test]
    async fn test_record_skill_execution() {
        let store = create_test_store().await;
        let skill = create_test_skill();
        store.save_skill(&skill).await.unwrap();

        store
            .record_skill_execution(skill.id, None, true, Some(100), &[])
            .await
            .unwrap();

        store
            .record_skill_execution(skill.id, None, false, Some(50), &[])
            .await
            .unwrap();

        let (total, successes) = store.get_skill_execution_count(skill.id).await.unwrap();
        assert_eq!(total, 2);
        assert_eq!(successes, 1);
    }

    #[tokio::test]
    async fn test_update_skill_metrics() {
        let store = create_test_store().await;
        let mut skill = create_test_skill();
        skill.activate();
        store.save_skill(&skill).await.unwrap();

        store
            .update_skill_metrics(skill.id, 10, 0.8, Some(150), "active")
            .await
            .unwrap();

        let updated = store.get_skill(skill.id).await.unwrap();
        assert_eq!(updated.metadata.usage_count, 10);
        assert!((updated.metadata.success_rate - 0.8).abs() < 0.01);
        assert_eq!(updated.metadata.avg_duration_ms, Some(150));
        assert_eq!(updated.status, SkillStatus::Active);
    }

    #[tokio::test]
    async fn test_update_skill_metrics_auto_disable() {
        let store = create_test_store().await;
        let mut skill = create_test_skill();
        skill.activate();
        store.save_skill(&skill).await.unwrap();

        // Simulate low success rate → disabled status
        store
            .update_skill_metrics(skill.id, 15, 0.2, Some(100), "disabled")
            .await
            .unwrap();

        let updated = store.get_skill(skill.id).await.unwrap();
        assert_eq!(updated.status, SkillStatus::Disabled);
        assert!((updated.metadata.success_rate - 0.2).abs() < 0.01);
    }
}
