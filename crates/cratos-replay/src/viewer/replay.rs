//! Viewer Replay - Rerun functionality for executions

use super::mod_impl::ExecutionViewer;
use super::types::{ReplayOptions, ReplayResult, ReplayStep};
use crate::error::Result;
use crate::event::{Event, EventType};
use crate::store::EventStoreTrait;
use tracing::instrument;
use uuid::Uuid;

impl ExecutionViewer {
    /// Replay an execution with the given options.
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
}
