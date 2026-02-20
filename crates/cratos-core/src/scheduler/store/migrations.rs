use super::SchedulerStore;
use crate::scheduler::types::{Result, SchedulerError};

impl SchedulerStore {
    /// Run database migrations
    pub(super) async fn migrate(&self) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(|e| SchedulerError::Transaction(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS scheduled_tasks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                trigger_json TEXT NOT NULL,
                action_json TEXT NOT NULL,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                priority INTEGER NOT NULL DEFAULT 0,
                max_retries INTEGER NOT NULL DEFAULT 3,
                created_at TIMESTAMP NOT NULL,
                updated_at TIMESTAMP NOT NULL,
                last_run_at TIMESTAMP,
                next_run_at TIMESTAMP,
                run_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| SchedulerError::Transaction(format!("Migration failed (scheduled_tasks): {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_executions (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                started_at TIMESTAMP NOT NULL,
                finished_at TIMESTAMP,
                status TEXT NOT NULL,
                result TEXT,
                attempt INTEGER NOT NULL DEFAULT 1,
                FOREIGN KEY (task_id) REFERENCES scheduled_tasks(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| SchedulerError::Transaction(format!("Migration failed (task_executions): {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_enabled ON scheduled_tasks(enabled)")
            .execute(&mut *tx)
            .await
            .map_err(|e| SchedulerError::Transaction(format!("Migration failed (idx_tasks_enabled): {}", e)))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_tasks_next_run ON scheduled_tasks(next_run_at)",
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| SchedulerError::Transaction(format!("Migration failed (idx_tasks_next_run): {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_executions_task ON task_executions(task_id)")
            .execute(&mut *tx)
            .await
            .map_err(|e| SchedulerError::Transaction(format!("Migration failed (idx_executions_task): {}", e)))?;

        tx.commit().await.map_err(|e| SchedulerError::Transaction(e.to_string()))?;

        Ok(())
    }
}
