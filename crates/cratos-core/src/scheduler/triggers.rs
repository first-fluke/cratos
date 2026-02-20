//! Scheduler triggers - conditions that activate scheduled tasks
//!
//! Supports multiple trigger types:
//! - Cron: time-based scheduling using cron expressions
//! - Interval: repeating tasks at fixed intervals
//! - OneTime: single execution at a specific time
//! - File: triggered by file system changes
//! - System: triggered by system resource thresholds

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Trigger types for scheduled tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerType {
    /// Cron-based scheduling (e.g., "0 9 * * *" for 9 AM daily)
    Cron(CronTrigger),
    /// Fixed interval scheduling
    Interval(IntervalTrigger),
    /// One-time execution at specific time
    OneTime(OneTimeTrigger),
    /// File system change trigger
    File(FileTrigger),
    /// System resource threshold trigger
    System(SystemTrigger),
}

impl TriggerType {
    /// Create a new cron trigger
    pub fn cron(expression: impl Into<String>) -> Self {
        Self::Cron(CronTrigger {
            expression: expression.into(),
            timezone: None,
        })
    }

    /// Create a new interval trigger
    pub fn interval(seconds: u64) -> Self {
        Self::Interval(IntervalTrigger {
            seconds,
            immediate: false,
        })
    }

    /// Create a one-time trigger
    pub fn one_time(at: DateTime<Utc>) -> Self {
        Self::OneTime(OneTimeTrigger { at })
    }

    /// Create a file change trigger
    pub fn file(path: impl Into<String>) -> Self {
        Self::File(FileTrigger {
            path: path.into(),
            events: vec![FileEvent::Modified],
            debounce_ms: 500,
        })
    }

    /// Create a CPU threshold trigger
    pub fn cpu_threshold(threshold_percent: f32) -> Self {
        Self::System(SystemTrigger {
            metric: SystemMetric::CpuUsage,
            threshold: threshold_percent,
            comparison: Comparison::GreaterThan,
            duration_secs: 60,
        })
    }

    /// Create a memory threshold trigger
    pub fn memory_threshold(threshold_percent: f32) -> Self {
        Self::System(SystemTrigger {
            metric: SystemMetric::MemoryUsage,
            threshold: threshold_percent,
            comparison: Comparison::GreaterThan,
            duration_secs: 60,
        })
    }

    /// Create a disk threshold trigger
    pub fn disk_threshold(threshold_percent: f32) -> Self {
        Self::System(SystemTrigger {
            metric: SystemMetric::DiskUsage,
            threshold: threshold_percent,
            comparison: Comparison::GreaterThan,
            duration_secs: 0,
        })
    }
}

/// Cron-based trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronTrigger {
    /// Cron expression (5 or 6 fields)
    /// Format: "minute hour day month weekday [year]"
    /// Examples:
    ///   "0 9 * * *" - Every day at 9:00 AM
    ///   "*/15 * * * *" - Every 15 minutes
    ///   "0 0 * * 1" - Every Monday at midnight
    pub expression: String,
    /// Optional timezone (default: UTC)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

/// Interval-based trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntervalTrigger {
    /// Interval in seconds
    pub seconds: u64,
    /// Execute immediately on start
    #[serde(default)]
    pub immediate: bool,
}

/// One-time trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimeTrigger {
    /// Execution time (UTC)
    pub at: DateTime<Utc>,
}

/// File system trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTrigger {
    /// Path to watch (supports glob patterns)
    pub path: String,
    /// Events to watch for
    #[serde(default = "default_file_events")]
    pub events: Vec<FileEvent>,
    /// Debounce time in milliseconds
    #[serde(default = "default_debounce")]
    pub debounce_ms: u64,
}

fn default_file_events() -> Vec<FileEvent> {
    vec![FileEvent::Modified, FileEvent::Created]
}

fn default_debounce() -> u64 {
    500
}

/// File system events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileEvent {
    /// File created
    Created,
    /// File modified
    Modified,
    /// File deleted
    Deleted,
    /// File renamed
    Renamed,
}

/// System resource trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemTrigger {
    /// Metric to monitor
    pub metric: SystemMetric,
    /// Threshold value
    pub threshold: f32,
    /// Comparison operator
    pub comparison: Comparison,
    /// Duration in seconds the condition must be true
    #[serde(default = "default_duration")]
    pub duration_secs: u64,
}

fn default_duration() -> u64 {
    60
}

/// System metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemMetric {
    /// CPU usage percentage (0-100)
    CpuUsage,
    /// Memory usage percentage (0-100)
    MemoryUsage,
    /// Disk usage percentage (0-100)
    DiskUsage,
    /// Network bytes received per second
    NetworkRx,
    /// Network bytes sent per second
    NetworkTx,
}

/// Comparison operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    /// Greater than threshold
    GreaterThan,
    /// Less than threshold
    LessThan,
    /// Equal to threshold (with epsilon)
    Equal,
}

impl Comparison {
    /// Check if value satisfies the comparison
    pub fn check(&self, value: f32, threshold: f32) -> bool {
        const EPSILON: f32 = 0.01;
        match self {
            Comparison::GreaterThan => value > threshold,
            Comparison::LessThan => value < threshold,
            Comparison::Equal => (value - threshold).abs() < EPSILON,
        }
    }
}

#[cfg(test)]
mod tests;

