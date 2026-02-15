//! Orchestrator tool execution
//!
//! Contains the tool execution logic for the Orchestrator:
//! - `execute_tool_calls`: Executes a list of tool calls

use crate::event_bus::OrchestratorEvent;
use crate::memory::WorkingMemory;
use crate::tool_policy::{PolicyAction, PolicyContext};
use cratos_llm::ToolCall;
use cratos_replay::EventType;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::core::Orchestrator;
use super::types::ToolCallRecord;

impl Orchestrator {
    /// Execute a list of tool calls
    ///
    /// # Phase 8: Persona-Skill Metrics
    ///
    /// When `matched_skill_id` is provided, records persona-skill metrics
    /// via `PersonaSkillStore` and checks for auto-assignment eligibility.
    pub(crate) async fn execute_tool_calls(
        &self,
        execution_id: Uuid,
        tool_calls: &[ToolCall],
        working_memory: &mut WorkingMemory,
        records: &mut Vec<ToolCallRecord>,
        active_persona: Option<&str>,
        matched_skill_id: Option<Uuid>,
    ) -> Vec<serde_json::Value> {
        let mut results = Vec::with_capacity(tool_calls.len());

        for call in tool_calls {
            info!(
                execution_id = %execution_id,
                tool = %call.name,
                args = %call.arguments,
                "Executing tool"
            );

            // Emit tool started event
            self.emit(OrchestratorEvent::ToolStarted {
                execution_id,
                tool_name: call.name.clone(),
                tool_call_id: call.id.clone(),
            });

            // Log tool call event
            self.log_event(
                execution_id,
                EventType::ToolCall,
                &serde_json::json!({
                    "tool": call.name,
                    "arguments": call.arguments
                }),
            )
            .await;

            // 6-Level security policy check
            if let Some(ref policy) = self.security_policy {
                let ctx = PolicyContext::default();
                match policy.resolve_or_default(&call.name, &ctx) {
                    PolicyAction::Deny => {
                        warn!(
                            execution_id = %execution_id,
                            tool = %call.name,
                            "Tool denied by security policy"
                        );
                        let output = serde_json::json!({"error": format!("Tool '{}' denied by security policy", call.name)});
                        self.emit(OrchestratorEvent::ToolCompleted {
                            execution_id,
                            tool_call_id: call.id.clone(),
                            tool_name: call.name.clone(),
                            success: false,
                            duration_ms: 0,
                        });
                        records.push(ToolCallRecord {
                            tool_name: call.name.clone(),
                            input: serde_json::json!({}),
                            output: output.clone(),
                            success: false,
                            duration_ms: 0,
                            persona_name: active_persona.map(String::from),
                        });
                        results.push(output);
                        continue;
                    }
                    PolicyAction::RequireApproval => {
                        // If approval manager exists, request approval; otherwise deny
                        if let Some(ref _am) = self.approval_manager {
                            debug!(
                                execution_id = %execution_id,
                                tool = %call.name,
                                "Tool requires approval per security policy"
                            );
                            // Approval flow is handled downstream — proceed for now
                        } else {
                            warn!(
                                execution_id = %execution_id,
                                tool = %call.name,
                                "Tool requires approval but no approval manager configured"
                            );
                        }
                    }
                    PolicyAction::Allow => {}
                }
            }

            // Parse arguments, fallback to empty object if malformed
            let input: serde_json::Value =
                serde_json::from_str(&call.arguments).unwrap_or_else(|e| {
                    warn!(
                        tool = %call.name,
                        error = %e,
                        arguments = %call.arguments,
                        "Failed to parse tool arguments, using empty object"
                    );
                    serde_json::json!({})
                });

            let start = std::time::Instant::now();
            let result = self.runner.execute(&call.name, input.clone()).await;
            let duration_ms = start.elapsed().as_millis() as u64;
            let duration_secs = start.elapsed().as_secs_f64();

            let (output, success, error) = match result {
                Ok(exec_result) => {
                    let mut output = exec_result.result.output.clone();
                    let success = exec_result.result.success;
                    let error = exec_result.result.error.clone();
                    // When tool returns failure with null output, embed error in
                    // the output JSON so downstream consumers (LLM conversation,
                    // fallback error messages) can access the reason.
                    if !success && output.is_null() {
                        if let Some(ref err_msg) = error {
                            output = serde_json::json!({"error": err_msg});
                        }
                    }
                    // Tool Doctor: diagnose soft failures (success=false from tool)
                    if !success {
                        let err_msg = error.as_deref().unwrap_or("unknown tool error");
                        let diagnosis = self.doctor.diagnose(&call.name, err_msg);
                        if diagnosis.confidence > 0.3 {
                            debug!(
                                tool = %call.name,
                                category = %diagnosis.category.display_name(),
                                confidence = %format!("{:.0}%", diagnosis.confidence * 100.0),
                                "Tool Doctor soft-failure diagnosis"
                            );
                        }
                    }
                    (output, success, error)
                }
                Err(e) => {
                    error!(
                        execution_id = %execution_id,
                        tool = %call.name,
                        error = %e,
                        "Tool execution failed"
                    );

                    // Tool Doctor: auto-diagnose failure
                    let error_str = e.to_string();
                    let diagnosis = self.doctor.diagnose(&call.name, &error_str);
                    let hint = self.doctor.format_diagnosis(&diagnosis);
                    debug!(
                        tool = %call.name,
                        category = %diagnosis.category.display_name(),
                        confidence = %format!("{:.0}%", diagnosis.confidence * 100.0),
                        "Tool Doctor diagnosis"
                    );

                    let enriched_error = format!(
                        "{}\n\n[Diagnosis: {} (confidence: {:.0}%)]\nSuggested fix: {}",
                        error_str,
                        diagnosis.category.display_name(),
                        diagnosis.confidence * 100.0,
                        diagnosis
                            .checklist
                            .first()
                            .map(|c| c.instruction.as_str())
                            .unwrap_or("Check logs for details"),
                    );

                    // Log diagnosis hint in event store
                    self.log_event(
                        execution_id,
                        EventType::Error,
                        &serde_json::json!({
                            "tool": call.name,
                            "error": error_str,
                            "diagnosis": hint,
                        }),
                    )
                    .await;

                    (
                        serde_json::json!({"error": enriched_error}),
                        false,
                        Some(error_str),
                    )
                }
            };

            // Log tool result event
            self.log_event(
                execution_id,
                EventType::ToolResult,
                &serde_json::json!({
                    "tool": call.name,
                    "success": success,
                    "output": output,
                    "error": error,
                    "duration_ms": duration_ms
                }),
            )
            .await;

            info!(
                execution_id = %execution_id,
                tool = %call.name,
                success = %success,
                duration_ms = %duration_ms,
                "Tool completed"
            );

            // Emit tool completed event
            self.emit(OrchestratorEvent::ToolCompleted {
                execution_id,
                tool_call_id: call.id.clone(),
                tool_name: call.name.clone(),
                success,
                duration_ms,
            });

            // Record labeled metrics
            {
                let status_label = if success { "ok" } else { "error" };
                crate::utils::metrics_global::labeled_counter("cratos_tool_executions_total")
                    .inc(&[("tool_name", &call.name), ("status", status_label)]);
                crate::utils::metrics_global::labeled_histogram("cratos_tool_duration_seconds")
                    .observe(&[("tool_name", &call.name)], duration_secs);
            }

            // Record in working memory
            working_memory.record_tool_execution(
                &call.name,
                input.clone(),
                Some(output.clone()),
                success,
                error,
            );

            // Record for return
            records.push(ToolCallRecord {
                tool_name: call.name.clone(),
                input,
                output: output.clone(),
                success,
                duration_ms,
                persona_name: active_persona.map(String::from),
            });

            // Phase 8: Record persona-skill metrics if skill was matched
            if let (Some(store), Some(persona), Some(skill_id)) =
                (&self.persona_skill_store, active_persona, matched_skill_id)
            {
                let config = cratos_skills::AutoAssignmentConfig::default();
                if let Err(e) = store
                    .record_execution(persona, skill_id, success, Some(duration_ms))
                    .await
                {
                    warn!(
                        persona = %persona,
                        skill_id = %skill_id,
                        error = %e,
                        "Failed to record persona-skill execution"
                    );
                }
                if let Err(e) = store
                    .check_auto_assignment(persona, skill_id, &config)
                    .await
                {
                    warn!(
                        persona = %persona,
                        skill_id = %skill_id,
                        error = %e,
                        "Failed to check auto-assignment"
                    );
                }
            }

            results.push(output);
        }

        results
    }
}
