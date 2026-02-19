use super::SkillStore;
use crate::error::{Error, Result};
use crate::skill::{Skill, SkillCategory};
use chrono::Utc;
use tracing::{debug, instrument};
use uuid::Uuid;

impl SkillStore {
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

    /// List stale skills (unused for N days)
    #[instrument(skip(self))]
    pub async fn list_stale_skills(&self, days: u32) -> Result<Vec<Skill>> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.to_rfc3339();

        let rows = sqlx::query(
            r#"
            SELECT * FROM skills
            WHERE origin NOT IN ('builtin', 'system')
            AND category != 'system'
            AND (
                last_used_at < ?1
                OR (last_used_at IS NULL AND updated_at < ?1)
            )
            ORDER BY last_used_at ASC
            "#,
        )
        .bind(cutoff_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        rows.into_iter().map(Self::row_to_skill).collect()
    }

    /// Prune stale skills
    #[instrument(skip(self))]
    pub async fn prune_stale_skills(&self, days: u32) -> Result<u64> {
        let stale_skills = self.list_stale_skills(days).await?;
        let count = stale_skills.len() as u64;

        if count == 0 {
            return Ok(0);
        }

        for skill in stale_skills {
            self.delete_skill(skill.id).await?;
        }

        Ok(count)
    }
}
