//! Post-execution handling for the Orchestrator
//!
//! Contains helper methods for session persistence, Graph RAG indexing,
//! metrics recording, and auto-skill detection.

use super::core::Orchestrator;
use super::sanitize::is_fake_tool_use_text;
use super::types::ToolCallRecord;
use cratos_llm::Message;
use std::sync::Arc;
use tracing::{debug, warn};

impl Orchestrator {
    /// Save session with execution summary
    ///
    /// Sanitizes session output to prevent pollution:
    /// - Tool failure details are NOT saved (prevents model learning failure patterns)
    /// - Fake tool-use text (from tool refusal) is replaced with a generic message
    pub(super) async fn save_session_with_summary(
        &self,
        session_key: &str,
        final_response: &str,
        tool_call_records: &[ToolCallRecord],
    ) {
        if let Ok(Some(mut session)) = self.memory.get(session_key).await {
            if !tool_call_records.is_empty() {
                // Only include tool names and OK/FAIL status — no error details.
                // Error details in session teach the model that tools fail, causing
                // it to avoid tool use or repeat the same failed patterns.
                let tool_summary: Vec<String> = tool_call_records
                    .iter()
                    .map(|r| {
                        if r.success {
                            format!("{}:OK", r.tool_name)
                        } else {
                            format!("{}:FAIL", r.tool_name)
                        }
                    })
                    .collect();
                let summary = format!(
                    "[Used {} tools: {}]\n{}",
                    tool_call_records.len(),
                    tool_summary.join(", "),
                    final_response
                );
                session.add_assistant_message(&summary);
            } else {
                // No tools used: check if the response is a fake tool-use pattern
                // (model wrote "[Used 1 tool: browser:OK]" as text instead of calling tools)
                let response = if is_fake_tool_use_text(final_response) {
                    debug!(
                        session_key = %session_key,
                        "Suppressing fake tool-use text from session"
                    );
                    "요청을 처리했습니다."
                } else {
                    final_response
                };
                session.add_assistant_message(response);
            }
            match self.memory.save(&session).await {
                Ok(()) => {
                    debug!(session_key = %session_key, messages = session.get_messages().len(), "Session saved (post-execution)")
                }
                Err(e) => {
                    warn!(session_key = %session_key, error = %e, "Failed to save session (post-execution)")
                }
            }
        }
    }

    /// Run Olympus OS post-execution hooks
    pub(super) fn run_post_execution_hooks(&self, effective_persona: &str, final_response: &str) {
        if let Some(hooks) = &self.olympus_hooks {
            let task_completed = !final_response.is_empty();
            if let Err(e) = hooks.post_execute(effective_persona, final_response, task_completed) {
                warn!(error = %e, "Olympus post-execute hook failed");
            }
        }
    }

    /// Index session messages with Graph RAG (async, fire-and-forget)
    pub(super) fn spawn_graph_rag_indexing(&self, session_key: &str, messages: &[Message]) {
        if let Some(gm) = &self.graph_memory {
            let gm = Arc::clone(gm);
            let sid = session_key.to_string();
            let msgs = messages.to_vec();
            tokio::spawn(async move {
                match gm.index_session(&sid, &msgs).await {
                    Ok(count) if count > 0 => {
                        debug!(session_id = %sid, indexed = count, "Graph RAG indexing complete");
                    }
                    Err(e) => warn!(error = %e, "Graph RAG indexing failed"),
                    _ => {}
                }
            });
        }
    }

    /// Trigger auto skill detection if enabled
    pub(super) fn spawn_auto_skill_detection(&self) {
        if self.config.auto_skill_detection {
            tokio::spawn(async move {
                match cratos_skills::analyzer::run_auto_analysis(false).await {
                    Ok(msg) => debug!("Auto-skill analysis: {}", msg),
                    Err(e) => warn!("Auto-skill analysis failed: {}", e),
                }
            });
        }
    }
}
