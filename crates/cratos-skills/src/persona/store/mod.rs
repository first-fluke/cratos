//! Persona-Skill Store
//!
//! SQLite-based storage for persona-skill bindings and execution tracking.
//! Enables persona-specific skill ownership, metrics, and auto-assignment.
//!
//! # Overview
//!
//! The [`PersonaSkillStore`] manages two tables:
//!
//! | Table | Description |
//! |-------|-------------|
//! | `persona_skills` | Persona-skill bindings with metrics |
//! | `persona_skill_executions` | Execution history for analytics |
//!
//! # Storage Location
//!
//! Uses the same database as [`SkillStore`]: `~/.cratos/skills.db`
//!
//! # Example
//!
//! ```ignore
//! use cratos_skills::{PersonaSkillStore, AutoAssignmentConfig};
//!
//! let store = PersonaSkillStore::from_path(&default_skill_db_path()).await?;
//!
//! // Claim a skill for a persona
//! store.claim_skill("sindri", skill_id, "api_builder").await?;
//!
//! // Record an execution
//! store.record_execution("sindri", skill_id, true, Some(150)).await?;
//!
//! // Check and apply auto-assignment
//! let config = AutoAssignmentConfig::default();
//! if store.check_auto_assignment("sindri", skill_id, &config).await? {
//!     println!("Skill auto-assigned to sindri!");
//! }
//!
//! // Get persona's top skills
//! let top = store.get_top_skills("sindri", 5).await?;
//! ```

use crate::error::{Error, Result};
use crate::persona::{
    AutoAssignmentConfig, OwnershipType, PersonaSkillBinding, PersonaSkillExecution,
};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::Row;
use std::path::Path;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// SQLite-based persona-skill store
#[derive(Clone)]
pub struct PersonaSkillStore {
    pool: SqlitePool,
}

impl PersonaSkillStore {
    /// Create a new store with an existing connection pool
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new store from a database path
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

        info!("PersonaSkillStore initialized at {}", db_path.display());
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

        debug!("In-memory PersonaSkillStore initialized");
        Ok(store)
    }

    /// Run database migrations
    async fn run_migrations(&self) -> Result<()> {
        // Persona-skill bindings table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS persona_skills (
                id TEXT PRIMARY KEY,
                persona_name TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                skill_name TEXT NOT NULL,
                ownership_type TEXT NOT NULL DEFAULT 'claimed',

                usage_count INTEGER NOT NULL DEFAULT 0,
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                success_rate REAL NOT NULL DEFAULT 1.0,
                avg_duration_ms INTEGER,
                last_used_at TEXT,

                consecutive_successes INTEGER NOT NULL DEFAULT 0,
                auto_assigned_at TEXT,

                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,

                UNIQUE(persona_name, skill_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Persona-skill executions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS persona_skill_executions (
                id TEXT PRIMARY KEY,
                persona_name TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                execution_id TEXT,
                success INTEGER NOT NULL,
                duration_ms INTEGER,
                error_message TEXT,
                started_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Create indexes
        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_persona_skills_persona ON persona_skills(persona_name)"#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_persona_skills_skill ON persona_skills(skill_id)"#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_persona_skill_execs_persona ON persona_skill_executions(persona_name)"#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_persona_skill_execs_skill ON persona_skill_executions(skill_id)"#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        debug!("PersonaSkillStore migrations completed");
        Ok(())
    }

    /// Get a reference to the connection pool
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // =========================================================================
    // Binding operations
    // =========================================================================

    /// Claim a skill for a persona (manual assignment)
    #[instrument(skip(self))]
    pub async fn claim_skill(
        &self,
        persona_name: &str,
        skill_id: Uuid,
        skill_name: &str,
    ) -> Result<PersonaSkillBinding> {
        // Check if binding already exists
        if let Some(existing) = self.get_binding(persona_name, skill_id).await? {
            debug!(
                "Skill {} already bound to {}: {}",
                skill_name, persona_name, existing.ownership_type
            );
            return Ok(existing);
        }

        let binding = PersonaSkillBinding::new(persona_name, skill_id, skill_name);
        self.save_binding(&binding).await?;

        info!(
            "Claimed skill '{}' for persona '{}'",
            skill_name, persona_name
        );
        Ok(binding)
    }

    /// Create a default binding (from TOML)
    #[instrument(skip(self))]
    pub async fn create_default_binding(
        &self,
        persona_name: &str,
        skill_id: Uuid,
        skill_name: &str,
    ) -> Result<PersonaSkillBinding> {
        // Check if binding already exists
        if let Some(existing) = self.get_binding(persona_name, skill_id).await? {
            return Ok(existing);
        }

        let binding = PersonaSkillBinding::default_binding(persona_name, skill_id, skill_name);
        self.save_binding(&binding).await?;

        debug!(
            "Created default binding for skill '{}' -> persona '{}'",
            skill_name, persona_name
        );
        Ok(binding)
    }

    /// Release a skill from a persona
    #[instrument(skip(self))]
    pub async fn release_skill(&self, persona_name: &str, skill_id: Uuid) -> Result<bool> {
        let result =
            sqlx::query(r#"DELETE FROM persona_skills WHERE persona_name = ?1 AND skill_id = ?2"#)
                .bind(persona_name)
                .bind(skill_id.to_string())
                .execute(&self.pool)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;

        let released = result.rows_affected() > 0;
        if released {
            info!("Released skill {} from persona {}", skill_id, persona_name);
        }
        Ok(released)
    }

    /// Save a binding (insert or update)
    #[instrument(skip(self, binding), fields(persona = %binding.persona_name, skill = %binding.skill_name))]
    pub async fn save_binding(&self, binding: &PersonaSkillBinding) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO persona_skills (
                id, persona_name, skill_id, skill_name, ownership_type,
                usage_count, success_count, failure_count, success_rate,
                avg_duration_ms, last_used_at, consecutive_successes, auto_assigned_at,
                created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13,
                ?14, ?15
            )
            ON CONFLICT(persona_name, skill_id) DO UPDATE SET
                skill_name = excluded.skill_name,
                ownership_type = excluded.ownership_type,
                usage_count = excluded.usage_count,
                success_count = excluded.success_count,
                failure_count = excluded.failure_count,
                success_rate = excluded.success_rate,
                avg_duration_ms = excluded.avg_duration_ms,
                last_used_at = excluded.last_used_at,
                consecutive_successes = excluded.consecutive_successes,
                auto_assigned_at = excluded.auto_assigned_at,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(binding.id.to_string())
        .bind(&binding.persona_name)
        .bind(binding.skill_id.to_string())
        .bind(&binding.skill_name)
        .bind(binding.ownership_type.as_str())
        .bind(i64::try_from(binding.usage_count).unwrap_or(i64::MAX))
        .bind(i64::try_from(binding.success_count).unwrap_or(i64::MAX))
        .bind(i64::try_from(binding.failure_count).unwrap_or(i64::MAX))
        .bind(binding.success_rate)
        .bind(
            binding
                .avg_duration_ms
                .map(|d| i64::try_from(d).unwrap_or(i64::MAX)),
        )
        .bind(binding.last_used_at.map(|t| t.to_rfc3339()))
        .bind(i32::try_from(binding.consecutive_successes).unwrap_or(i32::MAX))
        .bind(binding.auto_assigned_at.map(|t| t.to_rfc3339()))
        .bind(binding.created_at.to_rfc3339())
        .bind(binding.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    /// Get a binding by persona and skill ID
    #[instrument(skip(self))]
    pub async fn get_binding(
        &self,
        persona_name: &str,
        skill_id: Uuid,
    ) -> Result<Option<PersonaSkillBinding>> {
        let row = sqlx::query(
            r#"SELECT * FROM persona_skills WHERE persona_name = ?1 AND skill_id = ?2"#,
        )
        .bind(persona_name)
        .bind(skill_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        match row {
            Some(r) => Ok(Some(Self::row_to_binding(r)?)),
            None => Ok(None),
        }
    }

    /// Check if a persona has a skill by name
    #[instrument(skip(self))]
    pub async fn has_skill_by_name(&self, persona_name: &str, skill_name: &str) -> Result<bool> {
        let row = sqlx::query(
            r#"SELECT 1 FROM persona_skills WHERE persona_name = ?1 AND skill_name = ?2 LIMIT 1"#,
        )
        .bind(persona_name)
        .bind(skill_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.is_some())
    }

    /// Get all skills for a persona
    #[instrument(skip(self))]
    pub async fn get_persona_skills(&self, persona_name: &str) -> Result<Vec<PersonaSkillBinding>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM persona_skills
            WHERE persona_name = ?1
            ORDER BY success_rate DESC, usage_count DESC
            "#,
        )
        .bind(persona_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_binding).collect()
    }

    /// Get top N skills for a persona by success rate
    #[instrument(skip(self))]
    pub async fn get_top_skills(
        &self,
        persona_name: &str,
        limit: usize,
    ) -> Result<Vec<PersonaSkillBinding>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM persona_skills
            WHERE persona_name = ?1 AND usage_count >= 3
            ORDER BY success_rate DESC, usage_count DESC
            LIMIT ?2
            "#,
        )
        .bind(persona_name)
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_binding).collect()
    }

    /// Get skill leaderboard - which personas are best at a skill
    #[instrument(skip(self))]
    pub async fn get_skill_leaderboard(
        &self,
        skill_id: Uuid,
        limit: usize,
    ) -> Result<Vec<PersonaSkillBinding>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM persona_skills
            WHERE skill_id = ?1 AND usage_count >= 3
            ORDER BY success_rate DESC, usage_count DESC
            LIMIT ?2
            "#,
        )
        .bind(skill_id.to_string())
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_binding).collect()
    }

    /// Get all auto-assigned skills for a persona
    #[instrument(skip(self))]
    pub async fn get_auto_assigned_skills(
        &self,
        persona_name: &str,
    ) -> Result<Vec<PersonaSkillBinding>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM persona_skills
            WHERE persona_name = ?1 AND ownership_type = 'auto_assigned'
            ORDER BY auto_assigned_at DESC
            "#,
        )
        .bind(persona_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_binding).collect()
    }

    // =========================================================================
    // Execution tracking
    // =========================================================================

    /// Record a skill execution for a persona
    #[instrument(skip(self))]
    pub async fn record_execution(
        &self,
        persona_name: &str,
        skill_id: Uuid,
        success: bool,
        duration_ms: Option<u64>,
    ) -> Result<()> {
        self.record_execution_with_details(persona_name, skill_id, None, success, duration_ms, None)
            .await
    }

    /// Record a skill execution with full details
    #[instrument(skip(self))]
    pub async fn record_execution_with_details(
        &self,
        persona_name: &str,
        skill_id: Uuid,
        execution_id: Option<Uuid>,
        success: bool,
        duration_ms: Option<u64>,
        error_message: Option<&str>,
    ) -> Result<()> {
        // Record the execution
        let mut exec = PersonaSkillExecution::new(persona_name, skill_id);
        if let Some(eid) = execution_id {
            exec = exec.with_execution_id(eid);
        }
        if let Some(d) = duration_ms {
            exec = exec.with_duration(d);
        }
        if !success {
            exec = exec.failed(error_message.unwrap_or("unknown error"));
        }

        sqlx::query(
            r#"
            INSERT INTO persona_skill_executions (
                id, persona_name, skill_id, execution_id, success, duration_ms, error_message, started_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(exec.id.to_string())
        .bind(&exec.persona_name)
        .bind(exec.skill_id.to_string())
        .bind(exec.execution_id.map(|id| id.to_string()))
        .bind(exec.success)
        .bind(exec.duration_ms.map(|d| i64::try_from(d).unwrap_or(i64::MAX)))
        .bind(&exec.error_message)
        .bind(exec.started_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Update binding metrics
        if let Some(mut binding) = self.get_binding(persona_name, skill_id).await? {
            if success {
                binding.record_success(duration_ms.unwrap_or(0));
            } else {
                binding.record_failure();
            }
            self.save_binding(&binding).await?;
        } else {
            // Create implicit binding if it doesn't exist
            warn!(
                "Creating implicit binding for persona {} skill {}",
                persona_name, skill_id
            );
            let mut binding = PersonaSkillBinding::new(persona_name, skill_id, "unknown");
            if success {
                binding.record_success(duration_ms.unwrap_or(0));
            } else {
                binding.record_failure();
            }
            self.save_binding(&binding).await?;
        }

        debug!(
            "Recorded execution for {} skill {}: success={}",
            persona_name, skill_id, success
        );
        Ok(())
    }

    /// Check and apply auto-assignment if conditions are met
    #[instrument(skip(self, config))]
    pub async fn check_auto_assignment(
        &self,
        persona_name: &str,
        skill_id: Uuid,
        config: &AutoAssignmentConfig,
    ) -> Result<bool> {
        if !config.enabled {
            return Ok(false);
        }

        if let Some(mut binding) = self.get_binding(persona_name, skill_id).await? {
            if binding.qualifies_for_auto_assignment(config) {
                binding.mark_auto_assigned();
                self.save_binding(&binding).await?;

                info!(
                    "Auto-assigned skill '{}' to persona '{}' (consecutive={}, rate={:.2})",
                    binding.skill_name,
                    persona_name,
                    binding.consecutive_successes,
                    binding.success_rate
                );
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get execution history for a persona
    #[instrument(skip(self))]
    pub async fn get_execution_history(
        &self,
        persona_name: &str,
        limit: usize,
    ) -> Result<Vec<PersonaSkillExecution>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM persona_skill_executions
            WHERE persona_name = ?1
            ORDER BY started_at DESC
            LIMIT ?2
            "#,
        )
        .bind(persona_name)
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_execution).collect()
    }

    /// Get skill proficiency map for a persona (skill_name -> success_rate)
    #[instrument(skip(self))]
    pub async fn get_skill_proficiency_map(
        &self,
        persona_name: &str,
    ) -> Result<std::collections::HashMap<String, f64>> {
        let bindings = self.get_persona_skills(persona_name).await?;
        Ok(bindings
            .into_iter()
            .filter(|b| b.usage_count >= 3)
            .map(|b| (b.skill_name, b.success_rate))
            .collect())
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    fn row_to_binding(row: SqliteRow) -> Result<PersonaSkillBinding> {
        let id_str: String = row.get("id");
        let skill_id_str: String = row.get("skill_id");
        let ownership_str: String = row.get("ownership_type");
        let last_used_str: Option<String> = row.get("last_used_at");
        let auto_assigned_str: Option<String> = row.get("auto_assigned_at");
        let created_at_str: String = row.get("created_at");
        let updated_at_str: String = row.get("updated_at");

        let id = Uuid::parse_str(&id_str)
            .map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
        let skill_id = Uuid::parse_str(&skill_id_str)
            .map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
        let ownership_type: OwnershipType = ownership_str
            .parse()
            .map_err(|e: String| Error::Serialization(e))?;

        let last_used_at = last_used_str
            .map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))
            })
            .transpose()?;
        let auto_assigned_at = auto_assigned_str
            .map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))
            })
            .transpose()?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))?
            .with_timezone(&Utc);

        Ok(PersonaSkillBinding {
            id,
            persona_name: row.get("persona_name"),
            skill_id,
            skill_name: row.get("skill_name"),
            ownership_type,
            usage_count: u64::try_from(row.get::<i64, _>("usage_count")).unwrap_or(0),
            success_count: u64::try_from(row.get::<i64, _>("success_count")).unwrap_or(0),
            failure_count: u64::try_from(row.get::<i64, _>("failure_count")).unwrap_or(0),
            success_rate: row.get("success_rate"),
            avg_duration_ms: row
                .get::<Option<i64>, _>("avg_duration_ms")
                .map(|d| u64::try_from(d).unwrap_or(0)),
            last_used_at,
            consecutive_successes: u32::try_from(row.get::<i32, _>("consecutive_successes"))
                .unwrap_or(0),
            auto_assigned_at,
            created_at,
            updated_at,
        })
    }

    fn row_to_execution(row: SqliteRow) -> Result<PersonaSkillExecution> {
        let id_str: String = row.get("id");
        let skill_id_str: String = row.get("skill_id");
        let execution_id_str: Option<String> = row.get("execution_id");
        let started_at_str: String = row.get("started_at");

        let id = Uuid::parse_str(&id_str)
            .map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
        let skill_id = Uuid::parse_str(&skill_id_str)
            .map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))?;
        let execution_id = execution_id_str
            .map(|s| {
                Uuid::parse_str(&s).map_err(|e| Error::Serialization(format!("invalid uuid: {e}")))
            })
            .transpose()?;
        let started_at = DateTime::parse_from_rfc3339(&started_at_str)
            .map_err(|e| Error::Serialization(format!("invalid timestamp: {e}")))?
            .with_timezone(&Utc);

        Ok(PersonaSkillExecution {
            id,
            persona_name: row.get("persona_name"),
            skill_id,
            execution_id,
            success: row.get("success"),
            duration_ms: row
                .get::<Option<i64>, _>("duration_ms")
                .map(|d| u64::try_from(d).unwrap_or(0)),
            error_message: row.get("error_message"),
            started_at,
        })
    }
}

#[cfg(test)]
mod tests;
