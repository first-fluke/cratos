//! Scheduler task storage using SQLite
//!
//! Persists scheduled tasks for durability across restarts.

use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::path::Path;
use uuid::Uuid;

use super::types::{
    ExecutionRow, Result, ScheduledTask, SchedulerError, TaskExecution, TaskRow,
};

/// SQLite-based scheduler store
pub struct SchedulerStore {
    pool: Pool<Sqlite>,
}

impl SchedulerStore {
    /// Create a new store from database path
    pub async fn from_path(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SchedulerError::InvalidConfig(format!("Failed to create directory: {}", e))
            })?;
        }

        let url = format!("sqlite:{}?mode=rwc", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await?;

        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    /// Run database migrations
    async fn migrate(&self) -> Result<()> {
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
        .execute(&self.pool)
        .await?;

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
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_enabled ON scheduled_tasks(enabled)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_tasks_next_run ON scheduled_tasks(next_run_at)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_executions_task ON task_executions(task_id)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::triggers::TriggerType;
    use crate::scheduler::types::TaskAction;
    use tempfile::TempDir;

    struct TestContext {
        store: SchedulerStore,
        _dir: TempDir,
    }

    async fn create_test_context() -> TestContext {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test_scheduler.db");
        let store = SchedulerStore::from_path(&path).await.unwrap();
        TestContext { store, _dir: dir }
    }

    #[tokio::test]
    async fn test_create_and_get_task() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task = ScheduledTask::new(
            "test_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("Hello"),
        );

        store.create_task(&task).await.unwrap();

        let retrieved = store.get_task(task.id).await.unwrap();
        assert_eq!(retrieved.name, "test_task");
        assert_eq!(retrieved.id, task.id);
    }

    #[tokio::test]
    async fn test_update_task() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let mut task = ScheduledTask::new(
            "update_test",
            TriggerType::interval(3600),
            TaskAction::natural_language("Hello"),
        );

        store.create_task(&task).await.unwrap();

        task.name = "updated_name".to_string();
        task.enabled = false;

        store.update_task(&task).await.unwrap();

        let retrieved = store.get_task(task.id).await.unwrap();
        assert_eq!(retrieved.name, "updated_name");
        assert!(!retrieved.enabled);
    }

    #[tokio::test]
    async fn test_delete_task() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task = ScheduledTask::new(
            "delete_test",
            TriggerType::interval(3600),
            TaskAction::natural_language("Hello"),
        );

        store.create_task(&task).await.unwrap();
        store.delete_task(task.id).await.unwrap();

        let result = store.get_task(task.id).await;
        assert!(matches!(result, Err(SchedulerError::TaskNotFound(_))));
    }

    #[tokio::test]
    async fn test_list_enabled_tasks() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task1 = ScheduledTask::new(
            "enabled_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("Hello"),
        );

        let mut task2 = ScheduledTask::new(
            "disabled_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("World"),
        );
        task2.enabled = false;

        store.create_task(&task1).await.unwrap();
        store.create_task(&task2).await.unwrap();

        let enabled = store.list_enabled_tasks().await.unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "enabled_task");
    }

    #[tokio::test]
    async fn test_record_execution() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task = ScheduledTask::new(
            "exec_test",
            TriggerType::interval(3600),
            TaskAction::natural_language("Hello"),
        );

        store.create_task(&task).await.unwrap();

        let exec_id = store.record_execution_start(task.id, 1).await.unwrap();

        store
            .record_execution_complete(exec_id, "success", Some("Completed"))
            .await
            .unwrap();

        let executions = store.get_task_executions(task.id, 10).await.unwrap();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].status, "success");
        assert_eq!(executions[0].result, Some("Completed".to_string()));
    }

    #[test]
    fn test_task_action_natural_language() {
        let action = TaskAction::natural_language("Test prompt");
        match action {
            TaskAction::NaturalLanguage { prompt, channel } => {
                assert_eq!(prompt, "Test prompt");
                assert!(channel.is_none());
            }
            _ => panic!("Expected NaturalLanguage variant"),
        }
    }

    #[test]
    fn test_task_action_natural_language_with_channel() {
        let action = TaskAction::NaturalLanguage {
            prompt: "Test".to_string(),
            channel: Some("telegram".to_string()),
        };
        match action {
            TaskAction::NaturalLanguage { prompt, channel } => {
                assert_eq!(prompt, "Test");
                assert_eq!(channel, Some("telegram".to_string()));
            }
            _ => panic!("Expected NaturalLanguage variant"),
        }
    }

    #[test]
    fn test_task_action_notification() {
        let action = TaskAction::Notification {
            channel: "telegram".to_string(),
            channel_id: "123456789".to_string(),
            message: "Hello, World!".to_string(),
        };
        match action {
            TaskAction::Notification {
                channel,
                channel_id,
                message,
            } => {
                assert_eq!(channel, "telegram");
                assert_eq!(channel_id, "123456789");
                assert_eq!(message, "Hello, World!");
            }
            _ => panic!("Expected Notification variant"),
        }
    }

    #[test]
    fn test_task_action_webhook() {
        let action = TaskAction::Webhook {
            url: "https://example.com/webhook".to_string(),
            method: "POST".to_string(),
            headers: Some(serde_json::json!({"Content-Type": "application/json"})),
            body: Some(serde_json::json!({"key": "value"})),
        };
        match action {
            TaskAction::Webhook {
                url,
                method,
                headers,
                body,
            } => {
                assert_eq!(url, "https://example.com/webhook");
                assert_eq!(method, "POST");
                assert!(headers.is_some());
                assert!(body.is_some());
            }
            _ => panic!("Expected Webhook variant"),
        }
    }

    #[test]
    fn test_task_action_shell() {
        let action = TaskAction::Shell {
            command: "ls -la".to_string(),
            cwd: Some("/tmp".to_string()),
        };
        match action {
            TaskAction::Shell { command, cwd } => {
                assert_eq!(command, "ls -la");
                assert_eq!(cwd, Some("/tmp".to_string()));
            }
            _ => panic!("Expected Shell variant"),
        }
    }

    #[test]
    fn test_task_action_tool_call() {
        let action = TaskAction::ToolCall {
            tool: "web_search".to_string(),
            args: serde_json::json!({"query": "rust programming"}),
        };
        match action {
            TaskAction::ToolCall { tool, args } => {
                assert_eq!(tool, "web_search");
                assert_eq!(args["query"], "rust programming");
            }
            _ => panic!("Expected ToolCall variant"),
        }
    }

    #[test]
    fn test_task_action_serialization() {
        let action = TaskAction::Notification {
            channel: "slack".to_string(),
            channel_id: "C123".to_string(),
            message: "Test".to_string(),
        };

        let json = serde_json::to_string(&action).unwrap();
        let deserialized: TaskAction = serde_json::from_str(&json).unwrap();

        match deserialized {
            TaskAction::Notification { channel, .. } => {
                assert_eq!(channel, "slack");
            }
            _ => panic!("Deserialization failed"),
        }
    }

    #[test]
    fn test_webhook_method_variants() {
        for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
            let action = TaskAction::Webhook {
                url: "https://example.com".to_string(),
                method: method.to_string(),
                headers: None,
                body: None,
            };

            if let TaskAction::Webhook { method: m, .. } = action {
                assert_eq!(m, method);
            }
        }
    }

    #[tokio::test]
    async fn test_create_notification_task() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task = ScheduledTask::new(
            "notification_task",
            TriggerType::interval(3600),
            TaskAction::Notification {
                channel: "telegram".to_string(),
                channel_id: "12345".to_string(),
                message: "Scheduled notification".to_string(),
            },
        );

        store.create_task(&task).await.unwrap();

        let retrieved = store.get_task(task.id).await.unwrap();
        match retrieved.action {
            TaskAction::Notification { message, .. } => {
                assert_eq!(message, "Scheduled notification");
            }
            _ => panic!("Expected Notification action"),
        }
    }

    #[tokio::test]
    async fn test_create_webhook_task() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task = ScheduledTask::new(
            "webhook_task",
            TriggerType::interval(1800),
            TaskAction::Webhook {
                url: "https://api.example.com/hook".to_string(),
                method: "POST".to_string(),
                headers: Some(serde_json::json!({"Authorization": "Bearer token"})),
                body: Some(serde_json::json!({"event": "scheduled"})),
            },
        );

        store.create_task(&task).await.unwrap();

        let retrieved = store.get_task(task.id).await.unwrap();
        match retrieved.action {
            TaskAction::Webhook { url, method, .. } => {
                assert_eq!(url, "https://api.example.com/hook");
                assert_eq!(method, "POST");
            }
            _ => panic!("Expected Webhook action"),
        }
    }
}
