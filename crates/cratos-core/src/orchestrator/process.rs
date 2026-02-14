//! Orchestrator main execution loop
//!
//! Contains the `process` method - the main execution loop for the Orchestrator.

use crate::agents::extract_all_persona_mentions;
use crate::error::Result;
use crate::event_bus::OrchestratorEvent;
use crate::memory::WorkingMemory;
use crate::planner::Planner;
use cratos_llm::Message;
use cratos_memory::GraphMemory;
use cratos_replay::{EventType, Execution};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::config::OrchestratorInput;
use super::core::Orchestrator;
use super::sanitize::{is_tool_refusal, sanitize_error_for_user, sanitize_for_session_memory, sanitize_response};
use super::types::{ExecutionArtifact, ExecutionResult, ExecutionStatus};

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
            );
            // Override the auto-generated ID with our execution_id
            let mut execution = execution;
            execution.id = execution_id;
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

        // Get or create session
        let messages = {
            let mut session = match self.memory.get(&session_key).await {
                Ok(Some(s)) => {
                    debug!(session_key = %session_key, messages = s.get_messages().len(), "Session loaded");
                    s
                }
                Ok(None) => {
                    debug!(session_key = %session_key, "No existing session, creating new");
                    crate::memory::SessionContext::new(&session_key)
                }
                Err(e) => {
                    warn!(session_key = %session_key, error = %e, "Failed to load session, creating new");
                    crate::memory::SessionContext::new(&session_key)
                }
            };

            session.add_user_message(&input.text);

            // Graph RAG: always-on context enrichment
            if let Some(gm) = &self.graph_memory {
                let remaining = session.remaining_tokens();
                let total = session.token_count();

                if total > 0 && remaining < session.max_tokens / 5 {
                    // Token budget tight: REPLACE middle context
                    debug!(
                        remaining_tokens = remaining,
                        total_tokens = total,
                        "Token budget tight, replacing with Graph RAG context"
                    );
                    let budget = (session.max_tokens / 2) as u32;
                    match gm.retrieve(&input.text, 20, budget).await {
                        Ok(turns) if !turns.is_empty() => {
                            let retrieved_msgs = GraphMemory::turns_to_messages(&turns);
                            session.replace_with_retrieved(retrieved_msgs);
                            info!(
                                retrieved_turns = turns.len(),
                                "Replaced session context with Graph RAG results"
                            );
                        }
                        Ok(_) => debug!("No relevant Graph RAG turns found"),
                        Err(e) => warn!(error = %e, "Graph RAG retrieval failed"),
                    }
                } else {
                    // Normal: ADD supplementary context
                    let rag_budget = std::cmp::min((session.max_tokens / 10) as u32, 8000);
                    match gm.retrieve(&input.text, 5, rag_budget).await {
                        Ok(turns) if !turns.is_empty() => {
                            let retrieved_msgs = GraphMemory::turns_to_messages(&turns);
                            session.insert_supplementary_context(retrieved_msgs);
                            debug!(
                                retrieved_turns = turns.len(),
                                "Added supplementary RAG context"
                            );
                        }
                        Ok(_) => {}
                        Err(e) => warn!(error = %e, "Graph RAG supplementary retrieval failed"),
                    }
                }
            }

            // Explicit memory: auto-inject relevant saved memories
            if let Some(gm) = &self.graph_memory {
                match gm.recall_memories(&input.text, 3).await {
                    Ok(memories) if !memories.is_empty() => {
                        let memory_names: Vec<&str> =
                            memories.iter().map(|m| m.name.as_str()).collect();
                        let memory_context = memories
                            .iter()
                            .map(|m| format!("[Memory: {}] {}", m.name, m.content))
                            .collect::<Vec<_>>()
                            .join("\n");
                        session.insert_supplementary_context(vec![Message::system(
                            format!(
                                "Relevant saved memories (use these to help the user):\n{memory_context}"
                            ),
                        )]);
                        info!(
                            count = memories.len(),
                            names = ?memory_names,
                            "Injected explicit memories into context"
                        );
                    }
                    Ok(_) => {
                        debug!(query = %input.text, "No explicit memories matched");
                    }
                    Err(e) => warn!(error = %e, "Explicit memory recall failed"),
                }
            }

            // Save updated session
            match self.memory.save(&session).await {
                Ok(()) => {
                    debug!(session_key = %session_key, messages = session.get_messages().len(), "Session saved (pre-execution)")
                }
                Err(e) => {
                    warn!(session_key = %session_key, error = %e, "Failed to save session (pre-execution)")
                }
            }
            let mut msgs = session.get_messages().to_vec();

            // Attach inline images to the last user message (multimodal support)
            if !input.images.is_empty() {
                if let Some(last_user) = msgs
                    .iter_mut()
                    .rev()
                    .find(|m| m.role == cratos_llm::MessageRole::User)
                {
                    last_user.images = input.images.clone();
                    info!(
                        image_count = input.images.len(),
                        "Attached {} image(s) to user message",
                        input.images.len()
                    );
                }
            }

            msgs
        };

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
                    // No explicit persona: LLM semantic classification
                    _ => {
                        let name = self.route_by_llm(&input.text).await;
                        info!(persona = %name, "LLM-routed persona");
                        let prompt = mapping.get_system_prompt(&name, &input.user_id).map(|p| {
                            format!(
                                "{}\n\n---\n## Active Persona\n{}",
                                self.planner.config().system_prompt,
                                p
                            )
                        });
                        (prompt, name)
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
        let (skill_hint, matched_skill_id): (Option<String>, Option<Uuid>) =
            if let Some(router) = &self.skill_router {
                match router.route_best(&input.text).await {
                    Some(mut m) => {
                        // Apply persona skill proficiency bonus
                        if let Some(store) = &self.persona_skill_store {
                            let config = cratos_skills::AutoAssignmentConfig::default();
                            if let Ok(proficiency_map) =
                                store.get_skill_proficiency_map(&effective_persona).await
                            {
                                if let Some(&success_rate) = proficiency_map.get(&m.skill_name) {
                                    if success_rate >= config.proficiency_threshold {
                                        let old_score = m.score;
                                        m.score = (m.score + config.persona_skill_bonus).min(1.0);
                                        debug!(
                                            persona = %effective_persona,
                                            skill = %m.skill_name,
                                            old_score = %old_score,
                                            new_score = %m.score,
                                            success_rate = %success_rate,
                                            "Applied persona skill proficiency bonus"
                                        );
                                    }
                                }
                            }
                        }

                        // Only accept if score exceeds threshold (after bonus)
                        if m.score > 0.7 {
                            info!(
                                skill = %m.skill_name,
                                skill_id = %m.skill_id,
                                score = %m.score,
                                persona = %effective_persona,
                                "Skill match found"
                            );
                            (
                                Some(format!(
                                    "\n## Matched Skill: {}\n{}",
                                    m.skill_name, m.description
                                )),
                                Some(m.skill_id),
                            )
                        } else {
                            (None, None)
                        }
                    }
                    None => (None, None),
                }
            } else {
                (None, None)
            };

        // Combine system prompt overrides
        // input.system_prompt_override takes highest priority (e.g., /develop workflow)
        let effective_system_prompt: Option<String> = if let Some(ref override_prompt) =
            input.system_prompt_override
        {
            Some(override_prompt.clone())
        } else {
            match (persona_system_prompt, skill_hint) {
                (Some(p), Some(s)) => Some(format!("{}{}", p, s)),
                (Some(p), None) => Some(p),
                (None, Some(s)) => Some(format!("{}{}", self.planner.config().system_prompt, s)),
                (None, None) => None,
            }
        };

        // Create working memory
        let mut working_memory = WorkingMemory::with_execution_id(execution_id);
        let mut tool_call_records = Vec::new();
        let mut final_response = String::new();
        let mut model_used = None;
        let mut iteration = 0;
        let mut consecutive_all_fail = 0_usize;
        let mut total_failure_count = 0_usize;
        let mut hallucination_nudged = false;
        let mut fallback_sticky = false; // Once fallback is used, stick with it

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
                return Ok(ExecutionResult {
                    execution_id,
                    status: ExecutionStatus::Cancelled,
                    response: "실행이 취소되었습니다.".to_string(),
                    tool_calls: tool_call_records,
                    artifacts: Vec::new(),
                    iterations: iteration,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    model: model_used,
                });
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
                    &mut fallback_sticky,
                )
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    error!(execution_id = %execution_id, error = %e, "Planning failed");
                    self.log_event(
                        execution_id,
                        EventType::Error,
                        &serde_json::json!({
                            "error": e.to_string(),
                            "phase": "planning"
                        }),
                    )
                    .await;

                    let user_msg = match &e {
                        crate::error::Error::Llm(cratos_llm::Error::RateLimit) => {
                            "요청이 너무 많습니다. 잠시 후 다시 시도해주세요.".to_string()
                        }
                        crate::error::Error::Llm(cratos_llm::Error::Api(api_err))
                            if api_err.contains("INVALID_ARGUMENT") =>
                        {
                            warn!("Gemini INVALID_ARGUMENT (likely function call/response mismatch): {}", api_err);
                            "내부 처리 오류가 발생했습니다. 다시 시도해주세요.".to_string()
                        }
                        crate::error::Error::Llm(cratos_llm::Error::Api(api_err))
                            if api_err.contains("authentication") || api_err.contains("401") =>
                        {
                            "API 인증 오류가 발생했습니다. 관리자에게 문의해주세요.".to_string()
                        }
                        crate::error::Error::Llm(cratos_llm::Error::ServerError(_)) => {
                            "AI 서버에 일시적 장애가 발생했습니다. 잠시 후 다시 시도해주세요."
                                .to_string()
                        }
                        _ => {
                            let raw: String = e.to_string().chars().take(80).collect();
                            format!(
                                "오류가 발생했습니다. 다시 시도해주세요. ({})",
                                sanitize_error_for_user(&raw)
                            )
                        }
                    };
                    let error_detail = format!("Error: {}", e);
                    self.emit(OrchestratorEvent::ExecutionFailed {
                        execution_id,
                        error: error_detail.clone(),
                    });
                    // Update execution status in DB
                    if let Some(store) = &self.event_store {
                        let _ = store
                            .update_execution_status(execution_id, "failed", Some(&error_detail))
                            .await;
                    }
                    crate::utils::metrics_global::labeled_counter("cratos_executions_total")
                        .inc(&[("status", "failed")]);
                    crate::utils::metrics_global::gauge("cratos_active_executions").dec();
                    return Ok(ExecutionResult {
                        execution_id,
                        status: ExecutionStatus::Failed,
                        response: user_msg,
                        tool_calls: tool_call_records,
                        artifacts: Vec::new(),
                        iterations: iteration,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        model: model_used,
                    });
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
                        "위 작업을 위해 exec, http_get 등 도구를 사용해주세요. \
                         이 기기에서 직접 실행됩니다. 도구 없이 답하지 마세요.",
                    ));
                    continue;
                }

                // Detect hallucinated action: model returned a long final response
                // without using any tools. Ask the LLM classifier whether the
                // original request actually required tool execution.
                if !hallucination_nudged
                    && tool_call_records.is_empty()
                    && !is_tool_refusal(content_text)
                    && content_text.len() >= 20
                {
                    let needs_tools = self
                        .planner
                        .classify(
                            "Does this user request require executing tools \
                             (file operations, shell commands, web requests, API calls)? \
                             Answer only 'yes' or 'no'.",
                            &input.text,
                        )
                        .await
                        .unwrap_or_default();

                    if needs_tools.contains("yes") {
                        warn!(
                            execution_id = %execution_id,
                            "Model hallucinated tool use (tools_used=0, classify=yes), nudging retry"
                        );
                        hallucination_nudged = true;
                        messages.push(Message::assistant(content_text));
                        messages.push(Message::user(
                            "도구를 실제로 사용하지 않았습니다. 파일 생성/수정은 exec 도구를, \
                             웹 요청은 http_get 도구를, 검색은 web_search 도구를 사용해야 합니다. \
                             도구를 호출하여 실제로 작업을 수행해주세요.",
                        ));
                        continue;
                    }
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
                let results = self
                    .execute_tool_calls(
                        execution_id,
                        &filtered_calls,
                        &mut working_memory,
                        &mut tool_call_records,
                        Some(effective_persona.as_str()),
                        matched_skill_id,
                    )
                    .await;

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
                } else {
                    consecutive_all_fail = 0;
                }

                // Build tool result messages
                let tool_messages = Planner::build_tool_result_messages(&filtered_calls, &results);

                // Add tool result messages to conversation history for next iteration
                // NOTE: Only added to the local `messages` vec (for the current execution loop).
                // NOT persisted to session store individually — see post-loop summary below.
                messages.extend(tool_messages);
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
            self.update_persona_chronicle(
                &effective_persona,
                &final_response,
                execution_id,
            )
            .await;
        }

        // Sanitize response: strip leaked XML tags from weak models
        let final_response = sanitize_response(&final_response);

        // Generate fallback when LLM returns empty/sentinel after tool execution
        let final_response = if (final_response.is_empty() || final_response == "(empty response)")
            && !tool_call_records.is_empty()
        {
            let failed: Vec<&str> = tool_call_records
                .iter()
                .filter(|r| !r.success)
                .map(|r| r.tool_name.as_str())
                .collect();
            if failed.is_empty() {
                "요청을 처리하는 중 응답 생성에 실패했습니다. 다시 시도해주세요.".to_string()
            } else {
                let errors: Vec<String> = tool_call_records
                    .iter()
                    .filter(|r| !r.success)
                    .map(|r| {
                        r.output
                            .get("stderr")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .or_else(|| r.output.get("error").and_then(|v| v.as_str()))
                            .unwrap_or("unknown error")
                            .to_string()
                    })
                    .collect();
                // Deduplicate error messages (e.g. same "blocked" from exec+bash)
                let mut unique_errors: Vec<String> = Vec::new();
                for e in &errors {
                    if !unique_errors.iter().any(|u| u == e) {
                        unique_errors.push(e.clone());
                    }
                }
                // M3: Check if all errors are security blocks (expanded detection)
                let all_security = unique_errors.iter().all(|e| {
                    let lower = e.to_lowercase();
                    lower.contains("blocked")
                        || lower.contains("denied")
                        || lower.contains("forbidden")
                        || lower.contains("restricted")
                        || lower.contains("not allowed")
                        || lower.contains("unauthorized")
                });
                if all_security {
                    let reasons: String = unique_errors
                        .iter()
                        .map(|e| {
                            let short: String = e.chars().take(120).collect();
                            format!("- {}", short)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("보안 정책에 의해 해당 명령어가 차단되었습니다.\n{}\n\n안전한 대체 도구(http_get, http_post 등)를 사용해주세요.", reasons)
                } else {
                    let detail: String = unique_errors
                        .iter()
                        .map(|e| {
                            let short: String = e.chars().take(100).collect();
                            format!("- {}", sanitize_error_for_user(&short))
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("도구 실행에 실패했습니다:\n{}\n\n다른 방법으로 시도하거나 명령을 수정해 다시 요청해주세요.", detail)
                }
            }
        } else {
            final_response
        };

        // Update session with assistant response + execution summary
        // We persist a concise summary of tool usage (not individual tool messages)
        // to keep the session clean and avoid orphaned tool results that confuse LLMs.
        if let Ok(Some(mut session)) = self.memory.get(&session_key).await {
            if !tool_call_records.is_empty() {
                let tool_summary: Vec<String> = tool_call_records
                    .iter()
                    .map(|r| {
                        if r.success {
                            format!("{}:OK", r.tool_name)
                        } else {
                            let err_hint = r
                                .output
                                .get("stderr")
                                .and_then(|v| v.as_str())
                                .unwrap_or("failed");
                            let err_short: String = err_hint.chars().take(60).collect();
                            // M2: Sanitize to prevent prompt injection via session memory
                            format!(
                                "{}:FAIL({})",
                                r.tool_name,
                                sanitize_for_session_memory(&err_short)
                            )
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
                session.add_assistant_message(&final_response);
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

        // Run Olympus OS post-execution hooks (fire-and-forget)
        if let Some(hooks) = &self.olympus_hooks {
            let task_completed = !final_response.is_empty();
            if let Err(e) = hooks.post_execute(&effective_persona, &final_response, task_completed)
            {
                warn!(error = %e, "Olympus post-execute hook failed");
            }
        }

        // Graph RAG: index this session's messages asynchronously
        if let Some(gm) = &self.graph_memory {
            let gm = Arc::clone(gm);
            let sid = session_key.clone();
            let msgs = messages.clone();
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
        let mut artifacts = Vec::new();
        for record in &tool_call_records {
            // Check for screenshot
            if let Some(screenshot) = record.output.get("screenshot").and_then(|s| s.as_str()) {
                artifacts.push(ExecutionArtifact {
                    name: format!("{}_screenshot", record.tool_name),
                    mime_type: "image/png".to_string(), // Default assumes PNG
                    data: screenshot.to_string(),
                });
            }

            // Check for generic image output
            if let Some(image) = record.output.get("image").and_then(|s| s.as_str()) {
                artifacts.push(ExecutionArtifact {
                    name: format!("{}_image", record.tool_name),
                    mime_type: "image/png".to_string(),
                    data: image.to_string(),
                });
            }

            // Check for send_file artifact (structured artifact object)
            if let Some(artifact_obj) = record.output.get("artifact") {
                if let (Some(name), Some(mime_type), Some(data)) = (
                    artifact_obj.get("name").and_then(|v| v.as_str()),
                    artifact_obj.get("mime_type").and_then(|v| v.as_str()),
                    artifact_obj.get("data").and_then(|v| v.as_str()),
                ) {
                    artifacts.push(ExecutionArtifact {
                        name: name.to_string(),
                        mime_type: mime_type.to_string(),
                        data: data.to_string(),
                    });
                }
            }
        }

        // Record execution metrics
        crate::utils::metrics_global::labeled_counter("cratos_executions_total")
            .inc(&[("status", "completed")]);
        crate::utils::metrics_global::gauge("cratos_active_executions").dec();

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

use std::sync::Arc;
