//! Viewer Stats - Execution statistics and summaries

use super::mod_impl::ExecutionViewer;
use super::types::{truncate, ExecutionStats, ExecutionSummary};
use crate::event::{Event, EventType, Execution};

impl ExecutionViewer {
    pub(super) fn build_summary(
        &self,
        execution: &Execution,
        events: &[Event],
    ) -> ExecutionSummary {
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

    pub(super) fn calculate_stats(&self, events: &[Event]) -> ExecutionStats {
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
