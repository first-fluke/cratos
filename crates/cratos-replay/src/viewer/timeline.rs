//! Viewer Timeline - Timeline building and event summarization

use uuid::Uuid;
use crate::event::{Event, EventType, TimelineEntry};
use super::mod_impl::ExecutionViewer;
use super::types::truncate;

impl ExecutionViewer {
    pub(super) fn build_timeline(&self, events: &[Event]) -> Vec<TimelineEntry> {
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

    pub(super) fn summarize_event(&self, event: &Event) -> String {
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
}
