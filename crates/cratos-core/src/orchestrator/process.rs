//! Orchestrator main execution loop
//!
//! Contains the `process` method - the main execution loop for the Orchestrator.

use crate::agents::extract_all_persona_mentions;
use crate::error::Result;
use crate::event_bus::OrchestratorEvent;
use crate::memory::WorkingMemory;
use crate::planner::Planner;
use cratos_llm::Message;
use cratos_replay::{EventType, Execution};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::config::OrchestratorInput;
use super::core::Orchestrator;
use super::sanitize::{build_fallback_response, is_tool_refusal, sanitize_response};
use super::types::{ExecutionResult, ExecutionStatus};

impl Orchestrator {
    /// Process an input and return the result
    #[tracing::instrument(skip(self), fields(
        channel = %input.channel_type,
        user = %input.user_id
    ))]
    pub async fn process(&self, input: OrchestratorInput) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();
        let execution_id = Uuid::new_v4();
        let session_key = input.session_key();

        // Register cancellation token for this execution
        let cancel_token = CancellationToken::new();
        self.active_executions
            .insert(execution_id, cancel_token.clone());

        // Initialize steering context
        let mut steering_ctx = crate::steering::SteeringContext::new(execution_id);
        self.active_steer_handles
            .insert(execution_id, steering_ctx.handle());

        // Ensure steering handle cleanup
        struct SteerCleanup {
            handles: std::sync::Arc<dashmap::DashMap<Uuid, crate::steering::SteerHandle>>,
            id: Uuid,
        }
        impl Drop for SteerCleanup {
            fn drop(&mut self) {
                self.handles.remove(&self.id);
            }
        }
        let _steer_cleanup = SteerCleanup {
            handles: self.active_steer_handles.clone(),
            id: execution_id,
        };

        info!(
            execution_id = %execution_id,
            text = %input.text,
            "Starting execution"
        );

        // Record execution start metric
        crate::utils::metrics_global::gauge("cratos_active_executions").inc();

        // Emit execution started event
        self.emit(OrchestratorEvent::ExecutionStarted {
            execution_id,
            session_key: session_key.clone(),
        });

        // Create execution record in event store (required before logging events)
        if let Some(store) = &self.event_store {
            let execution = Execution::new(
                &input.channel_type,
                &input.channel_id,
                &input.user_id,
                &input.text,
            )
            .with_session_id(session_key.clone());

            // Override the auto-generated ID with our execution_id
            let mut execution = execution;
            execution.id = execution_id;

            if let Some(thread_id) = &input.thread_id {
                execution = execution.with_thread_id(thread_id);
            }

            if let Err(e) = store.create_execution(&execution).await {
                warn!(error = %e, "Failed to create execution record");
            }
        }

        // Log user input event
        self.log_event(
            execution_id,
            EventType::UserInput,
            &serde_json::json!({
                "channel_type": input.channel_type,
                "channel_id": input.channel_id,
                "user_id": input.user_id,
                "text": input.text
            }),
        )
        .await;

        // Get or create session with RAG context enrichment
        let messages = self.load_session_with_context(&session_key, &input).await;

        // ── Persona Routing (Phase 1 + Multi-Persona) ─────────────────
        // Memory-related requests bypass persona routing to ensure tool usage
        let is_memory_request = {
            let lower = input.text.to_lowercase();
            lower.contains("기억해")
                || lower.contains("저장해")
                || lower.contains("메모리")
                || lower.contains("remember")
                || lower.contains("recall")
                || lower.contains("save this")
        };

        let mut model_used: Option<String> = None;

        let (persona_system_prompt, effective_persona): (Option<String>, String) =
            if is_memory_request {
                info!("Memory request detected, using cratos directly");
                (None, "cratos".to_string())
            } else if let Some(mapping) = &self.persona_mapping {
                let multi_extraction = extract_all_persona_mentions(&input.text, mapping);

                match &multi_extraction {
                    // Multi-persona: delegate to execute_multi_persona
                    Some(e) if e.personas.len() > 1 => {
                        info!(
                            personas = ?e.personas.iter().map(|p| &p.name).collect::<Vec<_>>(),
                            mode = ?e.mode,
                            "Multi-persona routing"
                        );
                        // Early return with multi-persona execution
                        return self
                            .execute_multi_persona(
                                execution_id,
                                e.clone(),
                                &input,
                                &messages,
                                &cancel_token,
                            )
                            .await;
                    }
                    // Single persona: use existing logic
                    Some(e) if e.personas.len() == 1 => {
                        let mention = &e.personas[0].name;
                        info!(persona = %mention, "Explicit persona mention");
                        let prompt = mapping.get_system_prompt(mention, &input.user_id).map(|p| {
                            format!(
                                "{}\n\n---\n## Active Persona\n{}",
                                self.planner.config().system_prompt,
                                p
                            )
                        });
                        (prompt, mention.clone())
                    }
                    // No explicit persona: plan_step will adopt persona naturally
                    // via enhanced system prompt (persona awareness section).
                    // No separate LLM classify call needed.
                    _ => {
                        let name = "cratos".to_string();
                        debug!("No explicit mention, plan_step will adopt persona naturally via system prompt");
                        (None, name)
                    }
                }
            } else {
                let fb = self
                    .olympus_hooks
                    .as_ref()
                    .and_then(|h| h.active_persona())
                    .unwrap_or_else(|| "cratos".to_string());
                (None, fb)
            };

        // ── Skill Router (Phase 5) + Phase 8: skill_id extraction + persona bonus ─
        let skill_route = self.route_to_skill(&input.text, &effective_persona).await;
        let (skill_hint, matched_skill_id) = (skill_route.skill_hint, skill_route.skill_id);

        // Combine system prompt overrides
        let effective_system_prompt = self.combine_system_prompts(
            input.system_prompt_override.as_deref(),
            persona_system_prompt,
            skill_hint,
        );

        // Create working memory
        let mut working_memory = WorkingMemory::with_execution_id(execution_id);
        let mut tool_call_records = Vec::new();
        let mut final_response = String::new();
        let mut iteration = 0;
        let mut consecutive_all_fail = 0_usize;
        let mut total_failure_count = 0_usize;
        let mut fallback_sticky = false; // Once fallback is used, stick with it
        let mut continuation_nudged = false; // Nudge LLM to continue once if it stops mid-task

        // Messages accumulate tool call history across iterations
        let mut messages = messages;

        // Get available tools
        let tools = self.runner.registry().to_llm_tools();

        // Main execution loop
        loop {
            iteration += 1;
            if iteration > self.config.max_iterations {
                warn!(
                    execution_id = %execution_id,
                    iterations = %iteration,
                    "Max iterations reached, attempting final summary"
                );
                let summary = self
                    .try_final_summary(
                        &messages,
                        effective_system_prompt.as_deref(),
                        model_used.as_deref(),
                        fallback_sticky,
                    )
                    .await;
                if !summary.is_empty() {
                    final_response = summary;
                }
                break;
            }

            // ── Cancellation check ────────────────────────────────────
            if cancel_token.is_cancelled() {
                info!(execution_id = %execution_id, "Execution cancelled by user");
                self.active_executions.remove(&execution_id);
                if let Some(store) = &self.event_store {
                    let _ = store
                        .update_execution_status(execution_id, "cancelled", None)
                        .await;
                }
                crate::utils::metrics_global::labeled_counter("cratos_executions_total")
                    .inc(&[("status", "cancelled")]);
                crate::utils::metrics_global::gauge("cratos_active_executions").dec();
                return Ok(self.build_cancelled_result(
                    execution_id,
                    None,
                    tool_call_records,
                    iteration,
                    start_time.elapsed().as_millis() as u64,
                    model_used,
                ));
            }

            // ── Steering check ────────────────────────────────────────
            // Check for pending steering messages (e.g. user text from previous iteration)
            if let Some(msg) = steering_ctx.apply_after_tool().await {
                info!(execution_id = %execution_id, "Applying pending steering message");
                messages.push(cratos_llm::Message::user(format!(
                    "[User Intervention]: {}",
                    msg
                )));
            }

            // Check for new steering signals (Abort, etc.)
            match steering_ctx.check_before_tool().await {
                Ok(crate::steering::SteerDecision::Abort(reason)) => {
                    info!(execution_id = %execution_id, reason = ?reason, "Execution aborted by steering");
                    self.active_executions.remove(&execution_id);
                    // Cleanup DB status
                    if let Some(store) = &self.event_store {
                        let _ = store
                            .update_execution_status(execution_id, "cancelled", reason.as_deref())
                            .await;
                    }
                    crate::utils::metrics_global::labeled_counter("cratos_executions_total")
                        .inc(&[("status", "cancelled")]);
                    crate::utils::metrics_global::gauge("cratos_active_executions").dec();

                    return Ok(self.build_cancelled_result(
                        execution_id,
                        reason,
                        tool_call_records,
                        iteration,
                        start_time.elapsed().as_millis() as u64,
                        model_used,
                    ));
                }
                Ok(crate::steering::SteerDecision::Skip(_)) => {
                    // Skip implies skipping a tool call, but we are at the start of iteration.
                    // Just ignore for now.
                }
                Ok(crate::steering::SteerDecision::Continue) => {
                    // Check if a UserText was just processed and triggered Pending state
                    if let Some(msg) = steering_ctx.apply_after_tool().await {
                        info!(execution_id = %execution_id, "Applying new steering message");
                        messages.push(cratos_llm::Message::user(format!(
                            "[User Intervention]: {}",
                            msg
                        )));
                    }
                }
                Err(_) => {
                    // Channel closed or other error, ignore
                }
            }

            // ── Total execution timeout ───────────────────────────────
            if self.config.max_execution_secs > 0 {
                let elapsed = start_time.elapsed().as_secs();
                if elapsed >= self.config.max_execution_secs {
                    warn!(
                        execution_id = %execution_id,
                        elapsed_secs = elapsed,
                        limit_secs = self.config.max_execution_secs,
                        "Execution timeout reached"
                    );
                    if final_response.is_empty() {
                        let summary = self
                            .try_final_summary(
                                &messages,
                                effective_system_prompt.as_deref(),
                                model_used.as_deref(),
                                fallback_sticky,
                            )
                            .await;
                        final_response = if summary.is_empty() {
                            "처리 시간이 초과되었습니다. 요청을 단순화하거나 다시 시도해주세요."
                                .to_string()
                        } else {
                            summary
                        };
                    }
                    break;
                }
            }

            // ── Consecutive tool-failure bail-out ──────────────────────
            if consecutive_all_fail >= self.config.max_consecutive_failures {
                warn!(
                    execution_id = %execution_id,
                    consecutive_failures = consecutive_all_fail,
                    "Too many consecutive all-fail tool iterations, bailing out"
                );
                break;
            }

            // M4: Total failure bail-out (prevents reset-bypass via intermittent success)
            if total_failure_count >= self.config.max_total_failures {
                warn!(
                    execution_id = %execution_id,
                    total_failures = total_failure_count,
                    "Too many total tool failures, bailing out"
                );
                break;
            }

            debug!(
                execution_id = %execution_id,
                iteration = %iteration,
                "Planning step"
            );

            // Emit planning started event
            self.emit(OrchestratorEvent::PlanningStarted {
                execution_id,
                iteration,
            });

            // Plan the next step (with fallback and optional system prompt override)
            let plan_response = match self
                .plan_with_fallback(
                    &messages,
                    &tools,
                    effective_system_prompt.as_deref(),
                    model_used.as_deref(),
                    &mut fallback_sticky,
                )
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    self.active_executions.remove(&execution_id);
                    return Ok(self
                        .build_planning_failure_result(
                            execution_id,
                            &e,
                            tool_call_records,
                            iteration,
                            start_time.elapsed().as_millis() as u64,
                            model_used,
                        )
                        .await);
                }
            };

            model_used = Some(plan_response.model.clone());

            // Log LLM response event
            self.log_event(
                execution_id,
                EventType::LlmResponse,
                &serde_json::json!({
                    "content": plan_response.content,
                    "tool_calls": plan_response.tool_calls.len(),
                    "model": plan_response.model,
                    "is_final": plan_response.is_final
                }),
            )
            .await;

            // Check if this is a final response
            if plan_response.is_final {
                let content_text = plan_response.content.as_deref().unwrap_or("");

                // Detect tool refusal on first iteration: model says "can't access" instead of using tools.
                // Nudge it to retry with tools by appending a follow-up user message.
                if iteration == 1 && is_tool_refusal(content_text) {
                    warn!(
                        execution_id = %execution_id,
                        "Model refused to use tools on first iteration, nudging retry"
                    );
                    messages.push(Message::assistant(content_text));
                    messages.push(Message::user(
                        "거부하지 마세요. 당신은 이 기기에서 모든 작업을 수행할 권한이 있습니다. \
                         browser, exec, web_search 등 도구를 반드시 사용하세요. \
                         특정 사이트가 언급되면 browser로 직접 접속하세요. 도구 없이 답하지 마세요.",
                    ));
                    continue;
                }

                // If LLM returns empty/very short final after tool use, nudge it to complete
                if content_text.trim().is_empty()
                    && !tool_call_records.is_empty()
                    && iteration < self.config.max_iterations - 1
                {
                    warn!(
                        execution_id = %execution_id,
                        iteration = iteration,
                        "Model returned empty final response after tool use, nudging to complete"
                    );
                    messages.push(Message::user(
                        "도구 실행 결과를 바탕으로 원래 요청을 계속 수행해주세요. \
                         작업이 완료되지 않았다면 필요한 도구를 추가로 사용하고, \
                         완료했다면 결과를 사용자에게 설명해주세요.",
                    ));
                    continue;
                }

                // Nudge: LLM returned text-only mid-task (tools were used but task may be incomplete).
                // Push it once to re-check if all steps are done before accepting the response.
                if !continuation_nudged
                    && !tool_call_records.is_empty()
                    && iteration > 1
                    && iteration < self.config.max_iterations - 1
                    && !content_text.trim().is_empty()
                {
                    continuation_nudged = true;
                    warn!(
                        execution_id = %execution_id,
                        iteration = iteration,
                        "Model returned text-only mid-task, nudging to continue"
                    );
                    messages.push(Message::assistant(content_text));
                    messages.push(Message::user(
                        "The task may not be fully complete. Re-read the user's original request and check \
                         if any steps remain. If so, continue using tools to finish. \
                         Only report the final result once every step has been completed.",
                    ));
                    continue;
                }

                if let Some(content) = plan_response.content {
                    if !content.is_empty() {
                        self.emit(OrchestratorEvent::ChatDelta {
                            execution_id,
                            delta: content.clone(),
                            is_final: true,
                        });
                        final_response = content;
                    }
                }
                break;
            }

            // Execute tool calls
            if !plan_response.tool_calls.is_empty() {
                // Gate: block browser if http_get already succeeded in this execution
                let http_get_succeeded = tool_call_records
                    .iter()
                    .any(|r| r.tool_name == "http_get" && r.success);
                let filtered_calls: Vec<cratos_llm::ToolCall> = plan_response.tool_calls.iter().filter(|call| {
                    if call.name == "browser" && http_get_succeeded {
                        warn!(
                            execution_id = %execution_id,
                            "Blocking browser call — http_get already returned data. Use that instead."
                        );
                        false
                    } else {
                        true
                    }
                }).cloned().collect();

                // If all calls were filtered out, inject a synthetic nudge
                if filtered_calls.is_empty() {
                    messages.push(Message::assistant_with_tool_calls(
                        plan_response.content.clone().unwrap_or_default(),
                        plan_response.tool_calls.clone(),
                    ));
                    // Add fake tool results telling the model to use http_get data
                    let nudge_messages: Vec<Message> = plan_response.tool_calls.iter().map(|call| {
                        Message::tool_response_named(
                            &call.id,
                            &call.name,
                            "Browser is not available. Analyze the data from the previous http_get response instead. \
                             If you need more data, try http_get with a different URL.",
                        )
                    }).collect();
                    messages.extend(nudge_messages);
                    continue;
                }

                // Add assistant message with tool calls to conversation history
                messages.push(Message::assistant_with_tool_calls(
                    plan_response.content.clone().unwrap_or_default(),
                    filtered_calls.clone(),
                ));

                let pre_count = tool_call_records.len();
                let (results, steering_messages) = match self
                    .execute_tool_calls(
                        execution_id,
                        &filtered_calls,
                        &mut working_memory,
                        &mut tool_call_records,
                        Some(effective_persona.as_str()),
                        matched_skill_id,
                        &mut steering_ctx,
                    )
                    .await
                {
                    Ok(r) => r,
                    Err(crate::error::Error::Aborted(reason)) => {
                        info!(execution_id = %execution_id, reason = %reason, "Execution aborted by steering");
                        return Ok(ExecutionResult {
                            execution_id,
                            status: ExecutionStatus::Cancelled,
                            response: format!("실행이 중단되었습니다: {}", reason),
                            tool_calls: tool_call_records,
                            artifacts: Vec::new(),
                            iterations: iteration,
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            model: model_used,
                        });
                    }
                    Err(e) => return Err(e),
                };

                // Track consecutive all-fail iterations + total failures
                let new_records = &tool_call_records[pre_count..];
                let fail_count = new_records.iter().filter(|r| !r.success).count();
                total_failure_count += fail_count;
                let all_failed = !new_records.is_empty() && fail_count == new_records.len();
                if all_failed {
                    consecutive_all_fail += 1;
                    warn!(
                        execution_id = %execution_id,
                        consecutive_all_fail = consecutive_all_fail,
                        total_failures = total_failure_count,
                        "All tool calls failed this iteration"
                    );
                    // Reflection: inject a nudge when tools fail consecutively
                    if consecutive_all_fail >= 2 {
                        messages.push(Message::system(
                            "[reflection] The last tool calls failed consecutively. \
                             Do NOT repeat the same approach. Try a different tool or different parameters. \
                             If diagnosis messages suggest alternatives, use those instead."
                        ));
                    }
                } else {
                    consecutive_all_fail = 0;
                }

                // Build tool result messages
                let tool_messages = Planner::build_tool_result_messages(&filtered_calls, &results);

                // Add tool result messages to conversation history for next iteration
                // NOTE: Only added to the local `messages` vec (for the current execution loop).
                // NOT persisted to session store individually — see post-loop summary below.
                messages.extend(tool_messages);

                // Add steering messages (user interventions)
                for msg in steering_messages {
                    info!(execution_id = %execution_id, msg = %msg, "Injecting user intervention");
                    messages.push(Message::user(format!("[User Intervention]: {}", msg)));
                }
            }

            // If there's content along with tool calls, save it
            if let Some(content) = &plan_response.content {
                if !content.is_empty() {
                    final_response = content.clone();
                }
            }
        }

        // Log final response event
        self.log_event(
            execution_id,
            EventType::FinalResponse,
            &serde_json::json!({
                "response": final_response,
                "iterations": iteration,
                "tool_calls": tool_call_records.len()
            }),
        )
        .await;

        // Update persona chronicle with the final response/result
        if !final_response.is_empty() {
            self.update_persona_chronicle(&effective_persona, &final_response, execution_id)
                .await;
        }

        // Sanitize response: strip leaked XML tags from weak models
        let final_response = sanitize_response(&final_response);

        // Generate fallback when LLM returns empty/sentinel after tool execution
        let final_response = if (final_response.is_empty() || final_response == "(empty response)")
            && !tool_call_records.is_empty()
        {
            build_fallback_response(&tool_call_records)
        } else {
            final_response
        };

        // Update session with assistant response + execution summary
        self.save_session_with_summary(&session_key, &final_response, &tool_call_records)
            .await;

        // Run Olympus OS post-execution hooks (fire-and-forget)
        self.run_post_execution_hooks(&effective_persona, &final_response);

        // Graph RAG: index this session's messages asynchronously
        self.spawn_graph_rag_indexing(&session_key, &messages);

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Cleanup: remove from active executions
        self.active_executions.remove(&execution_id);

        // Emit execution completed event
        self.emit(OrchestratorEvent::ExecutionCompleted { execution_id });

        // Truncate response for logging (avoid flooding logs with huge responses)
        let response_preview: String = final_response
            .char_indices()
            .take_while(|(i, _)| *i < 200)
            .map(|(_, c)| c)
            .collect();
        info!(
            execution_id = %execution_id,
            iterations = %iteration,
            duration_ms = %duration_ms,
            tools_used = %tool_call_records.len(),
            response = %response_preview,
            "Execution completed"
        );

        // Update execution status in DB
        if let Some(store) = &self.event_store {
            if let Err(e) = store
                .update_execution_status(execution_id, "completed", Some(&final_response))
                .await
            {
                warn!(error = %e, "Failed to update execution status");
            }
        }

        // Extract artifacts from tool calls
        let artifacts = super::types::extract_artifacts(&tool_call_records);

        // Record execution metrics
        crate::utils::metrics_global::labeled_counter("cratos_executions_total")
            .inc(&[("status", "completed")]);
        crate::utils::metrics_global::gauge("cratos_active_executions").dec();

        // Trigger auto skill detection if enabled
        self.spawn_auto_skill_detection();

        Ok(ExecutionResult {
            execution_id,
            status: ExecutionStatus::Completed,
            response: final_response,
            tool_calls: tool_call_records,
            artifacts,
            iterations: iteration,
            duration_ms,
            model: model_used,
        })
    }
}
