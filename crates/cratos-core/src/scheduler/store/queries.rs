use super::SchedulerStore;
use crate::scheduler::types::{ExecutionRow, Result, ScheduledTask, SchedulerError, TaskExecution, TaskRow};
use chrono::{DateTime, Utc};
use uuid::Uuid;

impl SchedulerStore {
    /// Create a new scheduled task
    pub async fn create_task(&self, task: &ScheduledTask) -> Result<()> {
        let trigger_json = serde_json::to_string(&task.trigger)?;
        let action_json = serde_json::to_string(&task.action)?;

        sqlx::query(
            r#"
            INSERT INTO scheduled_tasks (
                id, name, description, trigger_json, action_json,
                enabled, priority, max_retries, created_at, updated_at,
                last_run_at, next_run_at, run_count, failure_count
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(task.id.to_string())
        .bind(&task.name)
        .bind(&task.description)
        .bind(trigger_json)
        .bind(action_json)
        .bind(task.enabled)
        .bind(task.priority)
        .bind(task.max_retries)
        .bind(task.created_at)
        .bind(task.updated_at)
        .bind(task.last_run_at)
        .bind(task.next_run_at)
        .bind(task.run_count)
        .bind(task.failure_count)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a task by ID
    pub async fn get_task(&self, id: Uuid) -> Result<ScheduledTask> {
        let row: TaskRow = sqlx::query_as("SELECT * FROM scheduled_tasks WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?
            .ok_or(SchedulerError::TaskNotFound(id))?;

        row.try_into()
    }

    /// Update a task
    pub async fn update_task(&self, task: &ScheduledTask) -> Result<()> {
        let trigger_json = serde_json::to_string(&task.trigger)?;
        let action_json = serde_json::to_string(&task.action)?;

        let result = sqlx::query(
            r#"
            UPDATE scheduled_tasks SET
                name = ?, description = ?, trigger_json = ?, action_json = ?,
                enabled = ?, priority = ?, max_retries = ?, updated_at = ?,
                last_run_at = ?, next_run_at = ?, run_count = ?, failure_count = ?
            WHERE id = ?
            "#,
        )
        .bind(&task.name)
        .bind(&task.description)
        .bind(trigger_json)
        .bind(action_json)
        .bind(task.enabled)
        .bind(task.priority)
        .bind(task.max_retries)
        .bind(Utc::now())
        .bind(task.last_run_at)
        .bind(task.next_run_at)
        .bind(task.run_count)
        .bind(task.failure_count)
        .bind(task.id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::TaskNotFound(task.id));
        }

        Ok(())
    }

    /// Delete a task
    pub async fn delete_task(&self, id: Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM scheduled_tasks WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(SchedulerError::TaskNotFound(id));
        }

        Ok(())
    }

    /// List all enabled tasks
    pub async fn list_enabled_tasks(&self) -> Result<Vec<ScheduledTask>> {
        let rows: Vec<TaskRow> = sqlx::query_as(
            "SELECT * FROM scheduled_tasks WHERE enabled = TRUE ORDER BY priority DESC, next_run_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }

    /// List all tasks
    pub async fn list_all_tasks(&self) -> Result<Vec<ScheduledTask>> {
        let rows: Vec<TaskRow> =
            sqlx::query_as("SELECT * FROM scheduled_tasks ORDER BY priority DESC, created_at DESC")
                .fetch_all(&self.pool)
                .await?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }

    /// Get tasks due for execution
    pub async fn get_due_tasks(&self, until: DateTime<Utc>) -> Result<Vec<ScheduledTask>> {
        let rows: Vec<TaskRow> = sqlx::query_as(
            r#"
            SELECT * FROM scheduled_tasks
            WHERE enabled = TRUE AND next_run_at IS NOT NULL AND next_run_at <= ?
            ORDER BY priority DESC, next_run_at ASC
            "#,
        )
        .bind(until)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }

    /// Record task execution start
    pub async fn record_execution_start(&self, task_id: Uuid, attempt: i32) -> Result<Uuid> {
        let execution_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO task_executions (id, task_id, started_at, status, attempt)
            VALUES (?, ?, ?, 'running', ?)
            "#,
        )
        .bind(execution_id.to_string())
        .bind(task_id.to_string())
        .bind(now)
        .bind(attempt)
        .execute(&self.pool)
        .await?;

        Ok(execution_id)
    }

    /// Record task execution completion
    pub async fn record_execution_complete(
        &self,
        execution_id: Uuid,
        status: &str,
        result: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE task_executions
            SET finished_at = ?, status = ?, result = ?
            WHERE id = ?
            "#,
        )
        .bind(Utc::now())
        .bind(status)
        .bind(result)
        .bind(execution_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get recent executions for a task
    pub async fn get_task_executions(
        &self,
        task_id: Uuid,
        limit: i64,
    ) -> Result<Vec<TaskExecution>> {
        let rows: Vec<ExecutionRow> = sqlx::query_as(
            r#"
            SELECT id, task_id, started_at, finished_at, status, result, attempt
            FROM task_executions
            WHERE task_id = ?
            ORDER BY started_at DESC
            LIMIT ?
            "#,
        )
        .bind(task_id.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }

    /// Update task run statistics
    pub async fn update_task_stats(
        &self,
        id: Uuid,
        last_run_at: DateTime<Utc>,
        next_run_at: Option<DateTime<Utc>>,
        success: bool,
    ) -> Result<()> {
        let (run_increment, failure_increment) = if success { (1, 0) } else { (1, 1) };

        sqlx::query(
            r#"
            UPDATE scheduled_tasks SET
                last_run_at = ?,
                next_run_at = ?,
                run_count = run_count + ?,
                failure_count = failure_count + ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(last_run_at)
        .bind(next_run_at)
        .bind(run_increment)
        .bind(failure_increment)
        .bind(Utc::now())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
