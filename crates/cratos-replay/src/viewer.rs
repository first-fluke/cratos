//! Viewer - Event query and replay API
//!
//! This module provides the query and visualization layer for the replay system.
//! It transforms raw events into human-readable timelines and summaries.

use crate::error::Result;
use crate::event::{Event, EventType, Execution, ExecutionStatus, TimelineEntry};
use crate::store::{EventStore, EventStoreTrait};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

/// Viewer for querying and displaying execution history
#[derive(Clone)]
pub struct ExecutionViewer {
    store: EventStore,
}

impl ExecutionViewer {
    /// Create a new execution viewer
    #[must_use]
    pub fn new(store: EventStore) -> Self {
        Self { store }
    }

    /// Get a detailed view of an execution with its timeline
    #[instrument(skip(self))]
    pub async fn get_execution_detail(&self, execution_id: Uuid) -> Result<ExecutionDetail> {
        let execution = self.store.get_execution(execution_id).await?;
        let events = self.store.get_execution_events(execution_id).await?;

        let timeline = self.build_timeline(&events);
        let summary = self.build_summary(&execution, &events);
        let stats = self.calculate_stats(&events);

        Ok(ExecutionDetail {
            execution,
            timeline,
            summary,
            stats,
        })
    }

    /// Get a simplified timeline for an execution
    #[instrument(skip(self))]
    pub async fn get_timeline(&self, execution_id: Uuid) -> Result<Vec<TimelineEntry>> {
        let events = self.store.get_execution_events(execution_id).await?;
        Ok(self.build_timeline(&events))
    }

    /// Get execution statistics
    #[instrument(skip(self))]
    pub async fn get_stats(&self, execution_id: Uuid) -> Result<ExecutionStats> {
        let events = self.store.get_execution_events(execution_id).await?;
        Ok(self.calculate_stats(&events))
    }

    /// Search executions by input text
    #[instrument(skip(self))]
    pub async fn search_executions(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<ExecutionSummary>> {
        // For now, just get recent executions and filter
        // In production, this would use full-text search
        let executions = self.store.list_recent_executions(limit * 2).await?;

        let filtered: Vec<_> = executions
            .into_iter()
            .filter(|e| e.input_text.to_lowercase().contains(&query.to_lowercase()))
            .take(limit as usize)
            .collect();

        let mut summaries = Vec::with_capacity(filtered.len());
        for execution in filtered {
            let events = self.store.get_execution_events(execution.id).await?;
            let summary = self.build_summary(&execution, &events);
            summaries.push(summary);
        }

        Ok(summaries)
    }

    /// Get recent execution summaries
    #[instrument(skip(self))]
    pub async fn get_recent_summaries(&self, limit: i64) -> Result<Vec<ExecutionSummary>> {
        let executions = self.store.list_recent_executions(limit).await?;

        let mut summaries = Vec::with_capacity(executions.len());
        for execution in executions {
            let events = self.store.get_execution_events(execution.id).await?;
            let summary = self.build_summary(&execution, &events);
            summaries.push(summary);
        }

        Ok(summaries)
    }

    /// Get executions in a time range
    #[instrument(skip(self))]
    pub async fn get_executions_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        channel_type: Option<&str>,
        channel_id: Option<&str>,
    ) -> Result<Vec<ExecutionSummary>> {
        // Get recent executions and filter by time range
        let executions = self.store.list_recent_executions(1000).await?;

        let filtered: Vec<_> = executions
            .into_iter()
            .filter(|e| {
                e.created_at >= from
                    && e.created_at <= to
                    && channel_type.is_none_or(|ct| e.channel_type == ct)
                    && channel_id.is_none_or(|ci| e.channel_id == ci)
            })
            .collect();

        let mut summaries = Vec::with_capacity(filtered.len());
        for execution in filtered {
            let events = self.store.get_execution_events(execution.id).await?;
            let summary = self.build_summary(&execution, &events);
            summaries.push(summary);
        }

        Ok(summaries)
    }

    /// Get the event chain for debugging (LLM requests and tool calls)
    #[instrument(skip(self))]
    pub async fn get_event_chain(&self, execution_id: Uuid) -> Result<EventChain> {
        let events = self.store.get_execution_events(execution_id).await?;

        let llm_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e.event_type, EventType::LlmRequest | EventType::LlmResponse))
            .cloned()
            .collect();

        let tool_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e.event_type, EventType::ToolCall | EventType::ToolResult))
            .cloned()
            .collect();

        let error_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == EventType::Error)
            .cloned()
            .collect();

        Ok(EventChain {
            execution_id,
            llm_events,
            tool_events,
            error_events,
        })
    }

    /// Replay an execution with the given options.
    ///
    /// In dry-run mode, returns the steps that *would* be executed without side effects.
    /// In live mode, records a new execution with the replayed events.
    #[instrument(skip(self))]
    pub async fn rerun(&self, execution_id: Uuid, options: ReplayOptions) -> Result<ReplayResult> {
        let _execution = self.store.get_execution(execution_id).await?;
        let events = self.store.get_execution_events(execution_id).await?;

        // Filter by sequence range
        let filtered: Vec<&Event> = events
            .iter()
            .filter(|e| {
                let seq = e.sequence_num;
                options.from_sequence.is_none_or(|from| seq >= from)
                    && options.to_sequence.is_none_or(|to| seq <= to)
            })
            .collect();

        let mut steps = Vec::with_capacity(filtered.len());

        for event in &filtered {
            let tool_name = event
                .payload
                .get("tool")
                .or_else(|| event.payload.get("tool_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let skipped = matches!(
                event.event_type,
                EventType::ToolCall | EventType::ToolResult
            ) && options.skip_tools.iter().any(|s| s == tool_name);

            let overridden = matches!(event.event_type, EventType::ToolCall)
                && options.tool_overrides.contains_key(tool_name);

            let replay_payload = if options.dry_run || skipped {
                None
            } else if overridden {
                // Use the override value as the replay payload
                options.tool_overrides.get(tool_name).cloned()
            } else {
                Some(event.payload.clone())
            };

            steps.push(ReplayStep {
                sequence: event.sequence_num,
                event_type: event.event_type,
                original_payload: event.payload.clone(),
                replay_payload,
                skipped,
                overridden,
            });
        }

        let new_execution_id = if options.dry_run {
            None
        } else {
            // In live mode, create a new execution record referencing the original
            let new_id = Uuid::new_v4();
            let mut new_exec = crate::event::Execution::new(
                &_execution.channel_type,
                &_execution.channel_id,
                &_execution.user_id,
                format!("[replay:{}] {}", execution_id, _execution.input_text),
            )
            .with_metadata(serde_json::json!({
                "replay_of": execution_id,
                "options": {
                    "from_sequence": options.from_sequence,
                    "to_sequence": options.to_sequence,
                    "skip_tools": options.skip_tools,
                    "tool_overrides_keys": options.tool_overrides.keys().collect::<Vec<_>>(),
                }
            }));

            if let Some(session_id) = &_execution.session_id {
                new_exec = new_exec.with_session_id(session_id);
            }

            if let Some(thread_id) = &_execution.thread_id {
                new_exec = new_exec.with_thread_id(thread_id);
            }

            new_exec.id = new_id;

            if let Err(e) = self.store.create_execution(&new_exec).await {
                tracing::warn!(error = %e, "Failed to create replay execution record");
            }

            // Record non-skipped events into the new execution
            for (i, step) in steps.iter().enumerate() {
                if step.skipped {
                    continue;
                }
                let payload = step
                    .replay_payload
                    .as_ref()
                    .unwrap_or(&step.original_payload);
                let event =
                    Event::new(new_id, i as i32, step.event_type).with_payload(payload.clone());
                if let Err(e) = self.store.append(event).await {
                    tracing::warn!(error = %e, "Failed to record replay event");
                }
            }

            // Mark the new execution as completed
            if let Err(e) = self
                .store
                .update_execution_status(new_id, "completed", None)
                .await
            {
                tracing::warn!(error = %e, "Failed to update replay execution status");
            }

            Some(new_id)
        };

        Ok(ReplayResult {
            original_execution_id: execution_id,
            new_execution_id,
            steps,
            dry_run: options.dry_run,
        })
    }

    /// Compare two executions (useful for debugging)
    #[instrument(skip(self))]
    pub async fn compare_executions(&self, id1: Uuid, id2: Uuid) -> Result<ExecutionComparison> {
        let detail1 = self.get_execution_detail(id1).await?;
        let detail2 = self.get_execution_detail(id2).await?;

        let diff = ExecutionDiff {
            input_same: detail1.execution.input_text == detail2.execution.input_text,
            output_same: detail1.execution.output_text == detail2.execution.output_text,
            status_same: detail1.execution.status == detail2.execution.status,
            tool_call_count_diff: detail1.stats.tool_call_count as i32
                - detail2.stats.tool_call_count as i32,
            llm_request_count_diff: detail1.stats.llm_request_count as i32
                - detail2.stats.llm_request_count as i32,
            duration_diff_ms: detail1
                .stats
                .total_duration_ms
                .zip(detail2.stats.total_duration_ms)
                .map(|(d1, d2)| d1 as i64 - d2 as i64),
        };

        Ok(ExecutionComparison {
            execution1: detail1,
            execution2: detail2,
            diff,
        })
    }

    // =========================================================================
    // Private helper methods
    // =========================================================================

    fn build_timeline(&self, events: &[Event]) -> Vec<TimelineEntry> {
        let child_parent_ids: std::collections::HashSet<Uuid> =
            events.iter().filter_map(|e| e.parent_event_id).collect();

        events
            .iter()
            .map(|event| {
                let summary = self.summarize_event(event);
                let has_children = child_parent_ids.contains(&event.id)
                    || events.iter().any(|e| e.parent_event_id == Some(event.id));

                TimelineEntry {
                    event_id: event.id,
                    timestamp: event.timestamp,
                    event_type: event.event_type,
                    summary,
                    duration_ms: event.duration_ms,
                    has_children,
                }
            })
            .collect()
    }

    fn summarize_event(&self, event: &Event) -> String {
        match event.event_type {
            EventType::UserInput => {
                let text = event
                    .payload
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                format!("User: {}", truncate(text, 50))
            }
            EventType::PlanCreated => {
                let steps = event
                    .payload
                    .get("steps")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                format!("Plan created with {} steps", steps)
            }
            EventType::LlmRequest => {
                let provider = event
                    .payload
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let model = event
                    .payload
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("LLM request to {}/{}", provider, model)
            }
            EventType::LlmResponse => {
                let provider = event
                    .payload
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let has_tools = event
                    .payload
                    .get("has_tool_calls")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if has_tools {
                    format!("LLM response from {} (with tool calls)", provider)
                } else {
                    format!("LLM response from {}", provider)
                }
            }
            EventType::ToolCall => {
                let tool = event
                    .payload
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("Tool call: {}", tool)
            }
            EventType::ToolResult => {
                let tool = event
                    .payload
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let success = event
                    .payload
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if success {
                    format!("Tool result: {} (success)", tool)
                } else {
                    format!("Tool result: {} (failed)", tool)
                }
            }
            EventType::FinalResponse => {
                let response = event
                    .payload
                    .get("response")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                format!("Response: {}", truncate(response, 50))
            }
            EventType::Error => {
                let message = event
                    .payload
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                format!("Error: {}", truncate(message, 50))
            }
            EventType::ApprovalRequested => "Approval requested".to_string(),
            EventType::ApprovalGranted => "Approval granted".to_string(),
            EventType::ApprovalDenied => "Approval denied".to_string(),
            EventType::Cancelled => "Execution cancelled".to_string(),
            EventType::ContextUpdated => "Context updated".to_string(),
        }
    }

    fn build_summary(&self, execution: &Execution, events: &[Event]) -> ExecutionSummary {
        let tool_calls: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == EventType::ToolCall)
            .filter_map(|e| e.payload.get("tool_name").and_then(|v| v.as_str()))
            .map(String::from)
            .collect();

        let has_errors = events.iter().any(|e| e.event_type == EventType::Error);

        let duration_ms = execution
            .completed_at
            .map(|completed| (completed - execution.started_at).num_milliseconds() as u64);

        ExecutionSummary {
            id: execution.id,
            channel_type: execution.channel_type.clone(),
            channel_id: execution.channel_id.clone(),
            user_id: execution.user_id.clone(),
            status: execution.status,
            input_preview: truncate(&execution.input_text, 100),
            output_preview: execution.output_text.as_deref().map(|s| truncate(s, 100)),
            tool_calls_used: tool_calls,
            has_errors,
            started_at: execution.started_at,
            duration_ms,
            event_count: events.len(),
        }
    }

    fn calculate_stats(&self, events: &[Event]) -> ExecutionStats {
        let llm_request_count = events
            .iter()
            .filter(|e| e.event_type == EventType::LlmRequest)
            .count();

        let tool_call_count = events
            .iter()
            .filter(|e| e.event_type == EventType::ToolCall)
            .count();

        let error_count = events
            .iter()
            .filter(|e| e.event_type == EventType::Error)
            .count();

        let total_llm_duration_ms: i64 = events
            .iter()
            .filter(|e| e.event_type == EventType::LlmResponse)
            .filter_map(|e| e.duration_ms)
            .map(i64::from)
            .sum();

        let total_tool_duration_ms: i64 = events
            .iter()
            .filter(|e| e.event_type == EventType::ToolResult)
            .filter_map(|e| e.duration_ms)
            .map(i64::from)
            .sum();

        let total_tokens: u64 = events
            .iter()
            .filter(|e| e.event_type == EventType::LlmResponse)
            .filter_map(|e| {
                e.payload
                    .get("tokens")
                    .and_then(|t| t.get("total_tokens"))
                    .and_then(|v| v.as_u64())
            })
            .sum();

        let total_duration_ms = if !events.is_empty() {
            let first = events.first().map(|e| e.timestamp);
            let last = events.last().map(|e| e.timestamp);
            first
                .zip(last)
                .map(|(f, l)| (l - f).num_milliseconds() as u64)
        } else {
            None
        };

        ExecutionStats {
            event_count: events.len(),
            llm_request_count,
            tool_call_count,
            error_count,
            total_llm_duration_ms: total_llm_duration_ms as u64,
            total_tool_duration_ms: total_tool_duration_ms as u64,
            total_tokens,
            total_duration_ms,
        }
    }
}

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
fn truncate(s: &str, max_len: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn test_replay_options_builder() {
        let opts = ReplayOptions::dry_run()
            .with_range(Some(1), Some(10))
            .skip(vec!["exec".to_string()]);

        assert!(opts.dry_run);
        assert_eq!(opts.from_sequence, Some(1));
        assert_eq!(opts.to_sequence, Some(10));
        assert_eq!(opts.skip_tools, vec!["exec"]);
    }

    #[test]
    fn test_replay_result_serialization() {
        let result = ReplayResult {
            original_execution_id: Uuid::nil(),
            new_execution_id: None,
            steps: vec![ReplayStep {
                sequence: 0,
                event_type: EventType::UserInput,
                original_payload: serde_json::json!({"text": "hello"}),
                replay_payload: None,
                skipped: false,
                overridden: false,
            }],
            dry_run: true,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"dry_run\":true"));
        assert!(json.contains("\"user_input\""));

        let deserialized: ReplayResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.dry_run);
        assert_eq!(deserialized.steps.len(), 1);
        assert!(deserialized.new_execution_id.is_none());
    }

    #[test]
    fn test_replay_step_skip_and_override() {
        let step_skipped = ReplayStep {
            sequence: 1,
            event_type: EventType::ToolCall,
            original_payload: serde_json::json!({"tool": "exec"}),
            replay_payload: None,
            skipped: true,
            overridden: false,
        };
        assert!(step_skipped.skipped);
        assert!(!step_skipped.overridden);

        let step_overridden = ReplayStep {
            sequence: 2,
            event_type: EventType::ToolCall,
            original_payload: serde_json::json!({"tool": "http_get"}),
            replay_payload: Some(serde_json::json!({"url": "http://new.example.com"})),
            skipped: false,
            overridden: true,
        };
        assert!(!step_overridden.skipped);
        assert!(step_overridden.overridden);
        assert!(step_overridden.replay_payload.is_some());
    }

    #[test]
    fn test_replay_options_with_overrides() {
        let mut overrides = std::collections::HashMap::new();
        overrides.insert(
            "http_get".to_string(),
            serde_json::json!({"url": "http://example.com"}),
        );

        let opts = ReplayOptions {
            dry_run: false,
            from_sequence: Some(2),
            to_sequence: Some(5),
            tool_overrides: overrides,
            skip_tools: vec!["exec".to_string()],
        };

        assert!(!opts.dry_run);
        assert_eq!(opts.from_sequence, Some(2));
        assert_eq!(opts.to_sequence, Some(5));
        assert!(opts.tool_overrides.contains_key("http_get"));
        assert_eq!(opts.skip_tools, vec!["exec"]);
    }
}
