//! Scheduler execution engine
//!
//! Manages the execution of scheduled tasks with:
//! - Cron scheduling
//! - Interval-based execution
//! - Graceful shutdown support
//! - Retry logic

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::store::SchedulerStore;
use super::types::{Result, ScheduledTask, SchedulerError, TaskAction};
use super::triggers::{IntervalTrigger, TriggerType};

/// Callback type for executing task actions
pub type TaskExecutor = Arc<dyn Fn(TaskAction) -> TaskExecutionFuture + Send + Sync>;

/// Future type for task execution
pub type TaskExecutionFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send>>;

/// Scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Check interval in seconds
    pub check_interval_secs: u64,
    /// Default retry delay in seconds
    pub retry_delay_secs: u64,
    /// Maximum concurrent task executions
    pub max_concurrent: usize,
    /// Enable execution logging
    pub logging_enabled: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 60,
            retry_delay_secs: 30,
            max_concurrent: 10,
            logging_enabled: true,
        }
    }
}

impl SchedulerConfig {
    /// Create a new configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set check interval
    pub fn with_check_interval(mut self, secs: u64) -> Self {
        self.check_interval_secs = secs;
        self
    }

    /// Set retry delay
    pub fn with_retry_delay(mut self, secs: u64) -> Self {
        self.retry_delay_secs = secs;
        self
    }

    /// Set max concurrent executions
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }
}

/// Internal state for running tasks
#[allow(dead_code)]
struct RunningTask {
    task_id: uuid::Uuid,
    started_at: DateTime<Utc>,
}

/// Scheduler engine for executing scheduled tasks
pub struct SchedulerEngine {
    store: Arc<SchedulerStore>,
    config: SchedulerConfig,
    running_tasks: Arc<RwLock<HashMap<uuid::Uuid, RunningTask>>>,
    executor: Option<TaskExecutor>,
}

impl SchedulerEngine {
    /// Create a new scheduler engine
    pub fn new(store: Arc<SchedulerStore>, config: SchedulerConfig) -> Self {
        Self {
            store,
            config,
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
            executor: None,
        }
    }

    /// Set the task executor callback
    pub fn with_executor(mut self, executor: TaskExecutor) -> Self {
        self.executor = Some(executor);
        self
    }

    /// Start the scheduler loop
    pub async fn run(&self, shutdown: CancellationToken) -> Result<()> {
        info!("Scheduler engine starting");

        // Load and schedule all enabled tasks
        self.initialize_tasks().await?;

        let check_interval = tokio::time::Duration::from_secs(self.config.check_interval_secs);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(check_interval) => {
                    if let Err(e) = self.check_and_execute().await {
                        error!("Scheduler check failed: {}", e);
                    }
                }
                _ = shutdown.cancelled() => {
                    info!("Scheduler engine shutting down");
                    break;
                }
            }
        }

        // Wait for running tasks to complete
        self.wait_for_running_tasks().await;

        info!("Scheduler engine stopped");
        Ok(())
    }

    /// Initialize tasks on startup
    async fn initialize_tasks(&self) -> Result<()> {
        let tasks = self.store.list_enabled_tasks().await?;
        let now = Utc::now();

        for mut task in tasks {
            // Calculate next run time if not set
            if task.next_run_at.is_none() {
                task.next_run_at = self.calculate_next_run(&task.trigger, now);
                self.store.update_task(&task).await?;
            }
        }

        info!(
            "Initialized {} scheduled tasks",
            self.store.list_enabled_tasks().await?.len()
        );
        Ok(())
    }

    /// Check for due tasks and execute them
    async fn check_and_execute(&self) -> Result<()> {
        let now = Utc::now();
        let due_tasks = self.store.get_due_tasks(now).await?;

        if due_tasks.is_empty() {
            debug!("No tasks due for execution");
            return Ok(());
        }

        let running = self.running_tasks.read().await;
        let running_count = running.len();
        drop(running);

        if running_count >= self.config.max_concurrent {
            warn!(
                "Max concurrent tasks reached ({}/{}), skipping this cycle",
                running_count, self.config.max_concurrent
            );
            return Ok(());
        }

        let available_slots = self.config.max_concurrent - running_count;
        let tasks_to_run: Vec<_> = due_tasks.into_iter().take(available_slots).collect();

        debug!("Executing {} due tasks", tasks_to_run.len());

        for task in tasks_to_run {
            self.execute_task(task).await;
        }

        Ok(())
    }

    /// Execute a single task
    async fn execute_task(&self, mut task: ScheduledTask) {
        let task_id = task.id;
        let task_name = task.name.clone();

        // Mark as running
        {
            let mut running = self.running_tasks.write().await;
            running.insert(
                task_id,
                RunningTask {
                    task_id,
                    started_at: Utc::now(),
                },
            );
        }

        if self.config.logging_enabled {
            info!("Executing scheduled task: {} ({})", task_name, task_id);
        }

        // Record execution start
        let execution_id = match self.store.record_execution_start(task_id, 1).await {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to record execution start: {}", e);
                self.cleanup_running_task(task_id).await;
                return;
            }
        };

        // Execute the action
        let result = self.execute_action(&task.action).await;

        // Record result
        let (status, result_str) = match &result {
            Ok(output) => ("success", Some(output.as_str())),
            Err(e) => {
                error!("Task {} failed: {}", task_name, e);
                ("failed", None)
            }
        };

        if let Err(e) = self
            .store
            .record_execution_complete(execution_id, status, result_str)
            .await
        {
            error!("Failed to record execution complete: {}", e);
        }

        // Update task statistics and next run time
        let now = Utc::now();
        let next_run = self.calculate_next_run(&task.trigger, now);
        task.last_run_at = Some(now);
        task.next_run_at = next_run;

        if let Err(e) = self
            .store
            .update_task_stats(task_id, now, next_run, result.is_ok())
            .await
        {
            error!("Failed to update task stats: {}", e);
        }

        if self.config.logging_enabled {
            info!("Task {} completed with status: {}", task_name, status);
        }

        self.cleanup_running_task(task_id).await;
    }

    /// Execute task action
    async fn execute_action(&self, action: &TaskAction) -> Result<String> {
        if let Some(executor) = &self.executor {
            executor(action.clone()).await
        } else {
            // Default stub implementation for testing
            match action {
                TaskAction::NaturalLanguage { prompt, .. } => {
                    debug!("Would execute prompt: {}", prompt);
                    Ok(format!("Executed: {}", prompt))
                }
                TaskAction::ToolCall { tool, args } => {
                    debug!("Would call tool: {} with args: {}", tool, args);
                    Ok(format!("Called tool: {}", tool))
                }
                TaskAction::Notification {
                    channel, message, ..
                } => {
                    debug!("Would send to {}: {}", channel, message);
                    Ok(format!("Notified: {}", channel))
                }
                TaskAction::Shell { command, .. } => {
                    debug!("Would execute: {}", command);
                    Ok(format!("Executed: {}", command))
                }
                TaskAction::Webhook { url, method, .. } => {
                    debug!("Would call {} {}", method, url);
                    Ok(format!("Called: {} {}", method, url))
                }
                TaskAction::RunSkillAnalysis { dry_run } => {
                    debug!("Would analyze skills (dry_run: {})", dry_run);
                    Ok(format!("Analyzed skills (dry_run: {})", dry_run))
                }
            }
        }
    }

    /// Calculate next run time based on trigger
    fn calculate_next_run(
        &self,
        trigger: &TriggerType,
        from: DateTime<Utc>,
    ) -> Option<DateTime<Utc>> {
        match trigger {
            TriggerType::Cron(cron) => self.calculate_next_cron(&cron.expression, from),
            TriggerType::Interval(IntervalTrigger { seconds, .. }) => {
                Some(from + Duration::seconds(*seconds as i64))
            }
            TriggerType::OneTime(one_time) => {
                if one_time.at > from {
                    Some(one_time.at)
                } else {
                    None // Already passed
                }
            }
            TriggerType::File(_) | TriggerType::System(_) => {
                // Event-driven triggers don't have a fixed next run time
                None
            }
        }
    }

    /// Calculate next cron execution time (simplified implementation)
    fn calculate_next_cron(&self, expression: &str, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
        // Parse simple cron expressions
        // Format: "minute hour day month weekday"
        let parts: Vec<&str> = expression.split_whitespace().collect();
        if parts.len() < 5 {
            warn!("Invalid cron expression: {}", expression);
            return None;
        }

        // For now, use a simplified approach:
        // Just add appropriate duration based on the pattern
        let minute = parts[0];
        let hour = parts[1];
        let day = parts[2];

        // Every minute
        if minute.starts_with("*/") && hour == "*" && day == "*" {
            if let Ok(interval) = minute.trim_start_matches("*/").parse::<i64>() {
                return Some(from + Duration::minutes(interval));
            }
        }

        // Every hour
        if minute != "*" && hour.starts_with("*/") && day == "*" {
            if let Ok(interval) = hour.trim_start_matches("*/").parse::<i64>() {
                return Some(from + Duration::hours(interval));
            }
        }

        // Daily (specific hour)
        if minute != "*" && hour != "*" && day == "*" {
            // Add 24 hours for daily tasks
            return Some(from + Duration::hours(24));
        }

        // Default: check again in 1 hour
        Some(from + Duration::hours(1))
    }

    /// Clean up running task record
    async fn cleanup_running_task(&self, task_id: uuid::Uuid) {
        let mut running = self.running_tasks.write().await;
        running.remove(&task_id);
    }

    /// Wait for all running tasks to complete
    async fn wait_for_running_tasks(&self) {
        let timeout = tokio::time::Duration::from_secs(30);
        let start = tokio::time::Instant::now();

        loop {
            let running = self.running_tasks.read().await;
            if running.is_empty() {
                break;
            }

            let count = running.len();
            drop(running);

            if start.elapsed() > timeout {
                warn!("Timeout waiting for {} running tasks", count);
                break;
            }

            info!("Waiting for {} running tasks to complete...", count);
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    /// Get the number of running tasks
    pub async fn running_count(&self) -> usize {
        self.running_tasks.read().await.len()
    }

    /// Add a new task
    pub async fn add_task(&self, task: ScheduledTask) -> Result<()> {
        let mut task = task;
        if task.next_run_at.is_none() {
            task.next_run_at = self.calculate_next_run(&task.trigger, Utc::now());
        }
        self.store.create_task(&task).await
    }

    /// Remove a task
    pub async fn remove_task(&self, task_id: uuid::Uuid) -> Result<()> {
        self.store.delete_task(task_id).await
    }

    /// Enable/disable a task
    pub async fn set_task_enabled(&self, task_id: uuid::Uuid, enabled: bool) -> Result<()> {
        let mut task = self.store.get_task(task_id).await?;
        task.enabled = enabled;
        if enabled && task.next_run_at.is_none() {
            task.next_run_at = self.calculate_next_run(&task.trigger, Utc::now());
        }
        self.store.update_task(&task).await
    }

    /// List all tasks
    pub async fn list_tasks(&self) -> Result<Vec<ScheduledTask>> {
        self.store.list_all_tasks().await
    }

    /// Get task by ID
    pub async fn get_task(&self, task_id: uuid::Uuid) -> Result<ScheduledTask> {
        self.store.get_task(task_id).await
    }
}

/// Builder for creating SchedulerEngine
pub struct SchedulerEngineBuilder {
    store: Option<Arc<SchedulerStore>>,
    config: SchedulerConfig,
    executor: Option<TaskExecutor>,
}

impl SchedulerEngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            store: None,
            config: SchedulerConfig::default(),
            executor: None,
        }
    }

    /// Set the store
    pub fn store(mut self, store: Arc<SchedulerStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Set the configuration
    pub fn config(mut self, config: SchedulerConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the executor
    pub fn executor(mut self, executor: TaskExecutor) -> Self {
        self.executor = Some(executor);
        self
    }

    /// Build the engine
    pub fn build(self) -> Result<SchedulerEngine> {
        let store = self
            .store
            .ok_or_else(|| SchedulerError::InvalidConfig("Store is required".to_string()))?;

        let mut engine = SchedulerEngine::new(store, self.config);
        if let Some(executor) = self.executor {
            engine = engine.with_executor(executor);
        }

        Ok(engine)
    }
}

impl Default for SchedulerEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct TestContext {
        engine: SchedulerEngine,
        _store: Arc<SchedulerStore>,
        _dir: TempDir,
    }

    async fn create_test_context() -> TestContext {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test_scheduler.db");
        let store = Arc::new(SchedulerStore::from_path(&path).await.unwrap());
        let config = SchedulerConfig::new().with_check_interval(1);
        let engine = SchedulerEngine::new(store.clone(), config);
        TestContext {
            engine,
            _store: store,
            _dir: dir,
        }
    }

    #[tokio::test]
    async fn test_add_and_list_tasks() {
        let ctx = create_test_context().await;

        let task = ScheduledTask::new(
            "test_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("Test prompt"),
        );

        ctx.engine.add_task(task).await.unwrap();

        let tasks = ctx.engine.list_tasks().await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "test_task");
    }

    #[tokio::test]
    async fn test_enable_disable_task() {
        let ctx = create_test_context().await;

        let task = ScheduledTask::new(
            "toggle_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("Test"),
        );
        let task_id = task.id;

        ctx.engine.add_task(task).await.unwrap();

        ctx.engine.set_task_enabled(task_id, false).await.unwrap();
        let task = ctx.engine.get_task(task_id).await.unwrap();
        assert!(!task.enabled);

        ctx.engine.set_task_enabled(task_id, true).await.unwrap();
        let task = ctx.engine.get_task(task_id).await.unwrap();
        assert!(task.enabled);
    }

    #[tokio::test]
    async fn test_remove_task() {
        let ctx = create_test_context().await;

        let task = ScheduledTask::new(
            "remove_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("Test"),
        );
        let task_id = task.id;

        ctx.engine.add_task(task).await.unwrap();
        ctx.engine.remove_task(task_id).await.unwrap();

        let result = ctx.engine.get_task(task_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_calculate_next_run_interval() {
        let ctx = create_test_context().await;

        let trigger = TriggerType::interval(3600);
        let now = Utc::now();
        let next = ctx.engine.calculate_next_run(&trigger, now);

        assert!(next.is_some());
        let next = next.unwrap();
        assert!(next > now);
        assert!((next - now).num_seconds() == 3600);
    }

    #[tokio::test]
    async fn test_running_count() {
        let ctx = create_test_context().await;
        assert_eq!(ctx.engine.running_count().await, 0);
    }
}
