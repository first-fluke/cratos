use super::SkillStore;
use crate::error::{Error, Result};
use chrono::Utc;
use sqlx::Row;
use tracing::{debug, instrument};
use uuid::Uuid;

impl SkillStore {
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
}
