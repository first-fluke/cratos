//! Scheduler task types and error definitions
//!
//! Contains the core types used by the scheduler system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

use super::triggers::TriggerType;

/// Result type for scheduler operations
pub type Result<T> = std::result::Result<T, SchedulerError>;

/// Scheduler error types
#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    /// Database error
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// Task not found
    #[error("task not found: {0}")]
    TaskNotFound(Uuid),
    /// Invalid configuration
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    /// Task execution error
    #[error("execution error: {0}")]
    Execution(String),
}

/// Scheduled task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// Unique task ID
    pub id: Uuid,
    /// Human-readable task name
    pub name: String,
    /// Task description
    pub description: Option<String>,
    /// Trigger configuration
    pub trigger: TriggerType,
    /// Action to execute
    pub action: TaskAction,
    /// Whether the task is enabled
    pub enabled: bool,
    /// Task priority (higher = more important)
    pub priority: i32,
    /// Maximum retry attempts
    pub max_retries: i32,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Last execution timestamp
    pub last_run_at: Option<DateTime<Utc>>,
    /// Next scheduled execution
    pub next_run_at: Option<DateTime<Utc>>,
    /// Total execution count
    pub run_count: i64,
    /// Failure count
    pub failure_count: i64,
}

impl ScheduledTask {
    /// Create a new scheduled task
    pub fn new(name: impl Into<String>, trigger: TriggerType, action: TaskAction) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            trigger,
            action,
            enabled: true,
            priority: 0,
            max_retries: 3,
            created_at: now,
            updated_at: now,
            last_run_at: None,
            next_run_at: None,
            run_count: 0,
            failure_count: 0,
        }
    }

    /// Set task description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set task priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set max retries
    pub fn with_max_retries(mut self, max_retries: i32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

/// Action to execute when triggered
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskAction {
    /// Execute a natural language prompt
    NaturalLanguage {
        /// The prompt to execute
        prompt: String,
        /// Optional channel to respond to
        channel: Option<String>,
    },
    /// Execute a specific tool
    ToolCall {
        /// Tool name
        tool: String,
        /// Tool arguments
        args: Value,
    },
    /// Send a notification
    Notification {
        /// Channel type (telegram, slack, etc.)
        channel: String,
        /// Channel ID
        channel_id: String,
        /// Message to send
        message: String,
    },
    /// Execute a shell command
    Shell {
        /// Command to execute
        command: String,
        /// Working directory
        cwd: Option<String>,
    },
    /// HTTP webhook
    Webhook {
        /// URL to call
        url: String,
        /// HTTP method
        method: String,
        /// Request headers
        headers: Option<Value>,
        /// Request body
        body: Option<Value>,
    },
    /// Analyze skills
    RunSkillAnalysis {
        /// Dry run mode
        dry_run: bool,
    },
}

impl TaskAction {
    /// Create a natural language action
    pub fn natural_language(prompt: impl Into<String>) -> Self {
        Self::NaturalLanguage {
            prompt: prompt.into(),
            channel: None,
        }
    }

    /// Create a tool call action
    pub fn tool_call(tool: impl Into<String>, args: Value) -> Self {
        Self::ToolCall {
            tool: tool.into(),
            args,
        }
    }

    /// Create a notification action
    pub fn notification(
        channel: impl Into<String>,
        channel_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Notification {
            channel: channel.into(),
            channel_id: channel_id.into(),
            message: message.into(),
        }
    }

    /// Create a skill analysis action
    pub fn run_skill_analysis(dry_run: bool) -> Self {
        Self::RunSkillAnalysis { dry_run }
    }
}

/// Task execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecution {
    /// Execution ID
    pub id: Uuid,
    /// Task ID
    pub task_id: Uuid,
    /// Start time
    pub started_at: DateTime<Utc>,
    /// End time
    pub finished_at: Option<DateTime<Utc>>,
    /// Execution status
    pub status: String,
    /// Result or error message
    pub result: Option<String>,
    /// Retry attempt number
    pub attempt: i32,
}

/// Internal row type for execution queries
#[derive(FromRow)]
pub(super) struct ExecutionRow {
    pub id: String,
    pub task_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub result: Option<String>,
    pub attempt: i32,
}

impl TryFrom<ExecutionRow> for TaskExecution {
    type Error = SchedulerError;

    fn try_from(row: ExecutionRow) -> Result<Self> {
        Ok(TaskExecution {
            id: Uuid::parse_str(&row.id).map_err(|e| {
                SchedulerError::InvalidConfig(format!("Invalid execution ID: {}", e))
            })?,
            task_id: Uuid::parse_str(&row.task_id)
                .map_err(|e| SchedulerError::InvalidConfig(format!("Invalid task ID: {}", e)))?,
            started_at: row.started_at,
            finished_at: row.finished_at,
            status: row.status,
            result: row.result,
            attempt: row.attempt,
        })
    }
}

/// Internal row type for database queries
#[derive(FromRow)]
pub(super) struct TaskRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub trigger_json: String,
    pub action_json: String,
    pub enabled: bool,
    pub priority: i32,
    pub max_retries: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub run_count: i64,
    pub failure_count: i64,
}

impl TryFrom<TaskRow> for ScheduledTask {
    type Error = SchedulerError;

    fn try_from(row: TaskRow) -> Result<Self> {
        Ok(ScheduledTask {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| SchedulerError::InvalidConfig(format!("Invalid task ID: {}", e)))?,
            name: row.name,
            description: row.description,
            trigger: serde_json::from_str(&row.trigger_json)?,
            action: serde_json::from_str(&row.action_json)?,
            enabled: row.enabled,
            priority: row.priority,
            max_retries: row.max_retries,
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_run_at: row.last_run_at,
            next_run_at: row.next_run_at,
            run_count: row.run_count,
            failure_count: row.failure_count,
        })
    }
}
