use super::SkillStore;
use crate::error::Result;
use tracing::debug;

impl SkillStore {
    /// Run database migrations
    pub(crate) async fn run_migrations(&self) -> Result<()> {
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
        .map_err(|e| crate::error::Error::Database(e.to_string()))?;

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
        .map_err(|e| crate::error::Error::Database(e.to_string()))?;

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
        .map_err(|e| crate::error::Error::Database(e.to_string()))?;

        // Create indexes
        sqlx::query(r#"CREATE INDEX IF NOT EXISTS idx_skills_status ON skills(status)"#)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::Database(e.to_string()))?;

        sqlx::query(r#"CREATE INDEX IF NOT EXISTS idx_skills_category ON skills(category)"#)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::Database(e.to_string()))?;

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_patterns_status ON detected_patterns(status)"#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Database(e.to_string()))?;

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_skill_executions_skill ON skill_executions(skill_id)"#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Database(e.to_string()))?;

        debug!("Skill store migrations completed");
        Ok(())
    }
}
