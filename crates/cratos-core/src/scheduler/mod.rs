//! ProactiveScheduler - 24/7 Task Scheduling System
//!
//! This module provides a comprehensive task scheduling system for Cratos,
//! enabling automated execution of tasks based on various triggers:
//!
//! - **Cron triggers**: Time-based scheduling using cron expressions
//! - **Interval triggers**: Fixed-interval repeating tasks
//! - **One-time triggers**: Single execution at a specific time
//! - **File triggers**: React to file system changes
//! - **System triggers**: React to system resource thresholds
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │ SchedulerEngine │  Main execution loop
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ SchedulerStore  │  SQLite persistence
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │  TriggerType    │  Trigger evaluation
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │   TaskAction    │  Action execution
//! └─────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use cratos_core::scheduler::{
//!     SchedulerEngine, SchedulerConfig, SchedulerStore,
//!     ScheduledTask, TaskAction, TriggerType,
//! };
//!
//! // Create store
//! let store = SchedulerStore::from_path(&db_path).await?;
//!
//! // Create engine
//! let config = SchedulerConfig::new()
//!     .with_check_interval(60)
//!     .with_max_concurrent(10);
//! let engine = SchedulerEngine::new(Arc::new(store), config);
//!
//! // Add a task
//! let task = ScheduledTask::new(
//!     "daily_backup",
//!     TriggerType::cron("0 3 * * *"),  // 3 AM daily
//!     TaskAction::natural_language("Backup all databases"),
//! );
//! engine.add_task(task).await?;
//!
//! // Run the scheduler
//! engine.run(shutdown_token).await?;
//! ```

mod engine;
mod store;
mod triggers;
mod types;

pub use engine::{SchedulerConfig, SchedulerEngine, SchedulerEngineBuilder, TaskExecutor};
pub use store::SchedulerStore;
pub use types::{
    Result as SchedulerResult, ScheduledTask, SchedulerError, TaskAction, TaskExecution,
};
pub use triggers::{
    Comparison, CronTrigger, FileEvent, FileTrigger, IntervalTrigger, OneTimeTrigger, SystemMetric,
    SystemTrigger, TriggerType,
};
