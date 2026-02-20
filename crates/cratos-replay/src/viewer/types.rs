//! Viewer Types - Common types for the execution viewer

use crate::event::{Event, EventType, Execution, ExecutionStatus, TimelineEntry};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Detailed view of an execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDetail {
    /// The execution record
    pub execution: Execution,
    /// Timeline of events
    pub timeline: Vec<TimelineEntry>,
    /// Summary information
    pub summary: ExecutionSummary,
    /// Statistics
    pub stats: ExecutionStats,
}

/// Summary view of an execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Execution ID
    pub id: Uuid,
    /// Channel type
    pub channel_type: String,
    /// Channel ID
    pub channel_id: String,
    /// User ID
    pub user_id: String,
    /// Status
    pub status: ExecutionStatus,
    /// Preview of input (truncated)
    pub input_preview: String,
    /// Preview of output (truncated)
    pub output_preview: Option<String>,
    /// Tools that were called
    pub tool_calls_used: Vec<String>,
    /// Whether any errors occurred
    pub has_errors: bool,
    /// When the execution started
    pub started_at: DateTime<Utc>,
    /// Total duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Number of events
    pub event_count: usize,
}

/// Statistics for an execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// Total number of events
    pub event_count: usize,
    /// Number of LLM requests
    pub llm_request_count: usize,
    /// Number of tool calls
    pub tool_call_count: usize,
    /// Number of errors
    pub error_count: usize,
    /// Total LLM latency in milliseconds
    pub total_llm_duration_ms: u64,
    /// Total tool execution time in milliseconds
    pub total_tool_duration_ms: u64,
    /// Total tokens used
    pub total_tokens: u64,
    /// Total execution duration in milliseconds
    pub total_duration_ms: Option<u64>,
}

/// Chain of events for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventChain {
    /// Execution ID
    pub execution_id: Uuid,
    /// LLM request/response events
    pub llm_events: Vec<Event>,
    /// Tool call/result events
    pub tool_events: Vec<Event>,
    /// Error events
    pub error_events: Vec<Event>,
}

/// Comparison between two executions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionComparison {
    /// First execution
    pub execution1: ExecutionDetail,
    /// Second execution
    pub execution2: ExecutionDetail,
    /// Differences
    pub diff: ExecutionDiff,
}

/// Differences between two executions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDiff {
    /// Whether inputs are the same
    pub input_same: bool,
    /// Whether outputs are the same
    pub output_same: bool,
    /// Whether statuses are the same
    pub status_same: bool,
    /// Difference in tool call count
    pub tool_call_count_diff: i32,
    /// Difference in LLM request count
    pub llm_request_count_diff: i32,
    /// Difference in duration
    pub duration_diff_ms: Option<i64>,
}

/// Result of a replay operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    /// Original execution ID
    pub original_execution_id: Uuid,
    /// New execution ID (None if dry_run)
    pub new_execution_id: Option<Uuid>,
    /// Steps processed during replay
    pub steps: Vec<ReplayStep>,
    /// Whether this was a dry run
    pub dry_run: bool,
}

/// A single step in the replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayStep {
    /// Sequence number in the original execution
    pub sequence: i32,
    /// Event type
    pub event_type: EventType,
    /// Original event payload
    pub original_payload: serde_json::Value,
    /// Replay result payload (None if dry_run or skipped)
    pub replay_payload: Option<serde_json::Value>,
    /// Whether this step was skipped
    pub skipped: bool,
    /// Whether tool input was overridden
    pub overridden: bool,
}

/// Replay options for re-executing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplayOptions {
    /// Whether to actually execute or just simulate (dry-run)
    pub dry_run: bool,
    /// Start from a specific event sequence number
    pub from_sequence: Option<i32>,
    /// Stop at a specific event sequence number
    pub to_sequence: Option<i32>,
    /// Override tool inputs
    #[serde(default)]
    pub tool_overrides: std::collections::HashMap<String, serde_json::Value>,
    /// Skip specific tools
    #[serde(default)]
    pub skip_tools: Vec<String>,
}

impl ReplayOptions {
    /// Create options for a dry-run replay
    #[must_use]
    pub fn dry_run() -> Self {
        Self {
            dry_run: true,
            ..Default::default()
        }
    }

    /// Set the event range
    #[must_use]
    pub fn with_range(mut self, from: Option<i32>, to: Option<i32>) -> Self {
        self.from_sequence = from;
        self.to_sequence = to;
        self
    }

    /// Skip specific tools
    #[must_use]
    pub fn skip(mut self, tools: Vec<String>) -> Self {
        self.skip_tools = tools;
        self
    }
}

/// Truncate a string to a maximum length, adding ellipsis if needed
pub(crate) fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let cut = max_len.saturating_sub(3);
        let safe_end = s
            .char_indices()
            .take_while(|(i, _)| *i < cut)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        format!("{}...", &s[..safe_end])
    }
}
