use super::SkillStore;
use crate::analyzer::{DetectedPattern, PatternStatus};
use crate::error::{Error, Result};
use tracing::{debug, instrument};
use uuid::Uuid;

impl SkillStore {
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
}
