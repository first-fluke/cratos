//! Orchestrator - Main execution loop
//!
//! This module provides the main orchestration logic that ties together
//! the planner, tools, memory, and replay systems.

use crate::agents::{extract_persona_mention, PersonaMapping};
use crate::approval::SharedApprovalManager;
use crate::error::Result;
use crate::event_bus::{EventBus, OrchestratorEvent};
use crate::memory::{MemoryStore, SessionContext, SessionStore, WorkingMemory};
use crate::olympus_hooks::OlympusHooks;
use crate::planner::{Planner, PlannerConfig};
use cratos_llm::{LlmProvider, Message, ToolCall, ToolDefinition};
use cratos_memory::GraphMemory;
use cratos_replay::{Event, EventStoreTrait, EventType, Execution};
use cratos_tools::{RunnerConfig, ToolRegistry, ToolRunner};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// System prompt for lightweight persona classification via LLM.
const PERSONA_CLASSIFICATION_PROMPT: &str = r#"Classify the user message into the most appropriate persona. Output ONLY the persona name, nothing else.

Personas:
- sindri: software development, coding, API, database, architecture, debugging, implementation
- athena: project management, planning, requirements, roadmap, sprint, schedule
- heimdall: QA, testing, security, code review, bug analysis, vulnerability
- mimir: research, investigation, analysis, comparison, documentation, study
- thor: DevOps, deployment, CI/CD, Docker, Kubernetes, infrastructure, server ops
- apollo: UX/UI design, user experience, prototyping, accessibility, wireframe
- odin: product ownership, vision, prioritization, OKR, stakeholder management
- cratos: general tasks, greetings, unclear domain, multi-domain, status, weather, casual

Rules:
- Output ONLY the persona name, nothing else
- If uncertain or multi-domain, output "cratos"
- Understand intent regardless of language (Korean, English, Japanese, etc.)"#;

/// Trait for routing user input to a matching skill
#[async_trait::async_trait]
pub trait SkillRouting: Send + Sync {
    /// Route input to the best matching skill.
    /// Returns `(skill_name, description, score)` if a match is found.
    async fn route_best(&self, input: &str) -> Option<(String, String, f32)>;
}

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    /// Execution is pending
    Pending,
    /// Execution is in progress
    Running,
    /// Execution completed successfully
    Completed,
    /// Execution failed
    Failed,
    /// Execution was cancelled
    Cancelled,
}

/// Result of an orchestrated execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Execution ID
    pub execution_id: Uuid,
    /// Final status
    pub status: ExecutionStatus,
    /// Response text
    pub response: String,
    /// Tool calls made
    pub tool_calls: Vec<ToolCallRecord>,
    /// Artifacts generated execution (e.g. screenshots, files)
    pub artifacts: Vec<ExecutionArtifact>,
    /// Total iterations
    pub iterations: usize,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Model used
    pub model: Option<String>,
}

/// Artifact generated during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionArtifact {
    /// Artifact name/id
    pub name: String,
    /// MIME type
    pub mime_type: String,
    /// Base64 encoded or raw data (represented as string for now)
    pub data: String,
}

/// Record of a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Tool name
    pub tool_name: String,
    /// Input arguments
    pub input: serde_json::Value,
    /// Output result
    pub output: serde_json::Value,
    /// Whether it succeeded
    pub success: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Input for orchestration
#[derive(Debug, Clone)]
pub struct OrchestratorInput {
    /// Channel type (e.g., "telegram", "slack")
    pub channel_type: String,
    /// Channel ID
    pub channel_id: String,
    /// User ID
    pub user_id: String,
    /// Thread ID (optional)
    pub thread_id: Option<String>,
    /// Input text
    pub text: String,
}

impl OrchestratorInput {
    /// Create a new input
    #[must_use]
    pub fn new(
        channel_type: impl Into<String>,
        channel_id: impl Into<String>,
        user_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            channel_type: channel_type.into(),
            channel_id: channel_id.into(),
            user_id: user_id.into(),
            thread_id: None,
            text: text.into(),
        }
    }

    /// Set the thread ID
    #[must_use]
    pub fn with_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Get the session key
    #[must_use]
    pub fn session_key(&self) -> String {
        SessionContext::make_key(&self.channel_type, &self.channel_id, &self.user_id)
    }
}

/// Configuration for the orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum iterations for tool calling
    pub max_iterations: usize,
    /// Whether to log events
    pub enable_logging: bool,
    /// Planner configuration
    pub planner_config: PlannerConfig,
    /// Runner configuration
    pub runner_config: RunnerConfig,
    /// Maximum total execution time in seconds (0 = no limit)
    pub max_execution_secs: u64,
    /// Bail out after this many consecutive iterations where ALL tool calls failed
    pub max_consecutive_failures: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            enable_logging: true,
            planner_config: PlannerConfig::default(),
            runner_config: RunnerConfig::default(),
            max_execution_secs: 90,
            max_consecutive_failures: 3,
        }
    }
}

impl OrchestratorConfig {
    /// Create a new configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum iterations
    #[must_use]
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set whether to enable event logging
    #[must_use]
    pub fn with_logging(mut self, enabled: bool) -> Self {
        self.enable_logging = enabled;
        self
    }

    /// Set planner configuration
    #[must_use]
    pub fn with_planner_config(mut self, config: PlannerConfig) -> Self {
        self.planner_config = config;
        self
    }

    /// Set runner configuration
    #[must_use]
    pub fn with_runner_config(mut self, config: RunnerConfig) -> Self {
        self.runner_config = config;
        self
    }
}

/// Main orchestrator that coordinates execution
pub struct Orchestrator {
    planner: Planner,
    runner: ToolRunner,
    memory: Arc<dyn SessionStore>,
    event_store: Option<Arc<dyn EventStoreTrait>>,
    event_bus: Option<Arc<EventBus>>,
    approval_manager: Option<SharedApprovalManager>,
    olympus_hooks: Option<OlympusHooks>,
    graph_memory: Option<Arc<GraphMemory>>,
    fallback_planner: Option<Planner>,
    persona_mapping: Option<PersonaMapping>,
    skill_router: Option<Arc<dyn SkillRouting>>,
    config: OrchestratorConfig,
}

impl Orchestrator {
    /// Create a new orchestrator
    #[must_use]
    pub fn new(
        llm_provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        config: OrchestratorConfig,
    ) -> Self {
        let planner = Planner::new(llm_provider, config.planner_config.clone());
        let runner = ToolRunner::new(tool_registry, config.runner_config.clone());

        Self {
            planner,
            runner,
            memory: Arc::new(MemoryStore::new()),
            event_store: None,
            event_bus: None,
            approval_manager: None,
            olympus_hooks: None,
            graph_memory: None,
            fallback_planner: None,
            persona_mapping: None,
            skill_router: None,
            config,
        }
    }

    /// Set the event store for logging
    pub fn with_event_store(mut self, store: Arc<dyn EventStoreTrait>) -> Self {
        self.event_store = Some(store);
        self
    }

    /// Set the event bus for real-time event broadcasting
    pub fn with_event_bus(mut self, bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Get a reference to the event bus (if set)
    #[must_use]
    pub fn event_bus(&self) -> Option<&Arc<EventBus>> {
        self.event_bus.as_ref()
    }

    /// Set the memory store
    pub fn with_memory(mut self, memory: Arc<dyn SessionStore>) -> Self {
        self.memory = memory;
        self
    }

    /// Set the approval manager for high-risk tool execution
    pub fn with_approval_manager(mut self, manager: SharedApprovalManager) -> Self {
        self.approval_manager = Some(manager);
        self
    }

    /// Set the Olympus OS hooks for post-execution processing
    pub fn with_olympus_hooks(mut self, hooks: OlympusHooks) -> Self {
        self.olympus_hooks = Some(hooks);
        self
    }

    /// Set the Graph RAG memory for long-term context retrieval
    pub fn with_graph_memory(mut self, graph_memory: Arc<GraphMemory>) -> Self {
        self.graph_memory = Some(graph_memory);
        self
    }

    /// Set the fallback LLM provider for rate-limit recovery
    pub fn with_fallback_provider(mut self, provider: Arc<dyn LlmProvider>) -> Self {
        let fallback_config = self.config.planner_config.clone();
        self.fallback_planner = Some(Planner::new(provider, fallback_config));
        self
    }

    /// Set the persona mapping for @mention and LLM-based routing
    pub fn with_persona_mapping(mut self, mapping: PersonaMapping) -> Self {
        self.persona_mapping = Some(mapping);
        self
    }

    /// Set the skill router for semantic skill matching
    pub fn with_skill_router(mut self, router: Arc<dyn SkillRouting>) -> Self {
        self.skill_router = Some(router);
        self
    }

    /// Get the tool runner
    #[must_use]
    pub fn runner(&self) -> &ToolRunner {
        &self.runner
    }

    /// Process an input and return the result
    #[instrument(skip(self), fields(
        channel = %input.channel_type,
        user = %input.user_id
    ))]
    pub async fn process(&self, input: OrchestratorInput) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();
        let execution_id = Uuid::new_v4();
        let session_key = input.session_key();

        info!(
            execution_id = %execution_id,
            text = %input.text,
            "Starting execution"
        );

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

            // Save updated session
            match self.memory.save(&session).await {
                Ok(()) => debug!(session_key = %session_key, messages = session.get_messages().len(), "Session saved (pre-execution)"),
                Err(e) => warn!(session_key = %session_key, error = %e, "Failed to save session (pre-execution)"),
            }
            session.get_messages().to_vec()
        };

        // ── Persona Routing (Phase 1) ──────────────────────────────────
        let (persona_system_prompt, effective_persona): (Option<String>, String) =
            if let Some(mapping) = &self.persona_mapping {
                let explicit = extract_persona_mention(&input.text, mapping);
                if let Some((_agent_id, _rest)) = explicit {
                    // @mention → highest priority
                    let mention = input.text.split_whitespace().next()
                        .unwrap_or("").trim_start_matches('@').to_lowercase();
                    info!(persona = %mention, "Explicit persona mention");
                    let prompt = mapping.get_system_prompt(&mention, &input.user_id)
                        .map(|p| format!("{}\n\n---\n## Active Persona\n{}",
                            self.planner.config().system_prompt, p));
                    (prompt, mention)
                } else {
                    // LLM semantic classification
                    let name = self.route_by_llm(&input.text).await;
                    info!(persona = %name, "LLM-routed persona");
                    let prompt = mapping.get_system_prompt(&name, &input.user_id)
                        .map(|p| format!("{}\n\n---\n## Active Persona\n{}",
                            self.planner.config().system_prompt, p));
                    (prompt, name)
                }
            } else {
                let fb = self.olympus_hooks.as_ref()
                    .and_then(|h| h.active_persona())
                    .unwrap_or_else(|| "cratos".to_string());
                (None, fb)
            };

        // ── Skill Router (Phase 5) ──────────────────────────────────────
        let skill_hint: Option<String> = if let Some(router) = &self.skill_router {
            router.route_best(&input.text).await
                .filter(|(_, _, score)| *score > 0.7)
                .map(|(name, desc, score)| {
                    info!(skill = %name, score = %score, "Skill match found");
                    format!("\n## Matched Skill: {}\n{}", name, desc)
                })
        } else {
            None
        };

        // Combine system prompt overrides
        let effective_system_prompt: Option<String> = match (persona_system_prompt, skill_hint) {
            (Some(p), Some(s)) => Some(format!("{}{}", p, s)),
            (Some(p), None) => Some(p),
            (None, Some(s)) => Some(format!("{}{}", self.planner.config().system_prompt, s)),
            (None, None) => None,
        };

        // Create working memory
        let mut working_memory = WorkingMemory::with_execution_id(execution_id);
        let mut tool_call_records = Vec::new();
        let mut final_response = String::new();
        let mut model_used = None;
        let mut iteration = 0;
        let mut consecutive_all_fail = 0_usize;

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
                    "Max iterations reached"
                );
                break;
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
                        final_response = "처리 시간이 초과되었습니다. 요청을 단순화하거나 다시 시도해주세요.".to_string();
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
            let plan_response = match self.plan_with_fallback(&messages, &tools, effective_system_prompt.as_deref()).await {
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
                            "AI 서버에 일시적 장애가 발생했습니다. 잠시 후 다시 시도해주세요.".to_string()
                        }
                        _ => {
                            format!("오류가 발생했습니다. 다시 시도해주세요. ({})",
                                e.to_string().chars().take(80).collect::<String>())
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
                         이 기기에서 직접 실행됩니다. 도구 없이 답하지 마세요."
                    ));
                    continue;
                }

                // If LLM returns empty/very short final after tool use, nudge it to complete
                if content_text.trim().is_empty() && !tool_call_records.is_empty() && iteration < self.config.max_iterations - 1 {
                    warn!(
                        execution_id = %execution_id,
                        iteration = iteration,
                        "Model returned empty final response after tool use, nudging to complete"
                    );
                    messages.push(Message::user(
                        "도구 실행 결과를 바탕으로 원래 요청을 계속 수행해주세요. \
                         작업이 완료되지 않았다면 필요한 도구를 추가로 사용하고, \
                         완료했다면 결과를 사용자에게 설명해주세요."
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
                let http_get_succeeded = tool_call_records.iter().any(|r| r.tool_name == "http_get" && r.success);
                let filtered_calls: Vec<ToolCall> = plan_response.tool_calls.iter().filter(|call| {
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
                    )
                    .await;

                // Track consecutive all-fail iterations
                let new_records = &tool_call_records[pre_count..];
                let all_failed = !new_records.is_empty() && new_records.iter().all(|r| !r.success);
                if all_failed {
                    consecutive_all_fail += 1;
                    warn!(
                        execution_id = %execution_id,
                        consecutive_all_fail = consecutive_all_fail,
                        "All tool calls failed this iteration"
                    );
                } else {
                    consecutive_all_fail = 0;
                }

                // Build tool result messages
                let tool_messages =
                    Planner::build_tool_result_messages(&filtered_calls, &results);

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

        // Sanitize response: strip leaked XML tags from weak models
        let final_response = sanitize_response(&final_response);

        // Generate fallback when LLM returns empty/sentinel after tool execution
        let final_response = if (final_response.is_empty() || final_response == "(empty response)") && !tool_call_records.is_empty() {
            let failed: Vec<&str> = tool_call_records.iter()
                .filter(|r| !r.success)
                .map(|r| r.tool_name.as_str())
                .collect();
            if failed.is_empty() {
                "요청을 처리하는 중 응답 생성에 실패했습니다. 다시 시도해주세요.".to_string()
            } else {
                let errors: Vec<String> = tool_call_records.iter()
                    .filter(|r| !r.success)
                    .map(|r| {
                        r.output.get("stderr").and_then(|v| v.as_str())
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
                // Check if all errors are security blocks
                let all_security = unique_errors.iter().all(|e|
                    e.contains("blocked") || e.contains("permission denied")
                        || e.contains("Permission denied")
                );
                if all_security {
                    let reasons: String = unique_errors.iter()
                        .map(|e| {
                            let short: String = e.chars().take(120).collect();
                            format!("- {}", short)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("보안 정책에 의해 해당 명령어가 차단되었습니다.\n{}\n\n안전한 대체 도구(http_get, http_post 등)를 사용해주세요.", reasons)
                } else {
                    let detail: String = unique_errors.iter()
                        .map(|e| {
                            let short: String = e.chars().take(100).collect();
                            format!("- {}", short)
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
                            let err_hint = r.output.get("stderr")
                                .and_then(|v| v.as_str())
                                .unwrap_or("failed");
                            let err_short: String = err_hint.chars().take(60).collect();
                            format!("{}:FAIL({})", r.tool_name, err_short)
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
                Ok(()) => debug!(session_key = %session_key, messages = session.get_messages().len(), "Session saved (post-execution)"),
                Err(e) => warn!(session_key = %session_key, error = %e, "Failed to save session (post-execution)"),
            }
        }

        // Run Olympus OS post-execution hooks (fire-and-forget)
        if let Some(hooks) = &self.olympus_hooks {
            let task_completed = !final_response.is_empty();
            if let Err(e) = hooks.post_execute(&effective_persona, &final_response, task_completed) {
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
                .update_execution_status(
                    execution_id,
                    "completed",
                    Some(&final_response),
                )
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
        }

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

    /// Execute a list of tool calls
    async fn execute_tool_calls(
        &self,
        execution_id: Uuid,
        tool_calls: &[ToolCall],
        working_memory: &mut WorkingMemory,
        records: &mut Vec<ToolCallRecord>,
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
                    (output, success, error)
                }
                Err(e) => {
                    error!(
                        execution_id = %execution_id,
                        tool = %call.name,
                        error = %e,
                        "Tool execution failed"
                    );
                    (
                        serde_json::json!({"error": e.to_string()}),
                        false,
                        Some(e.to_string()),
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
            });

            results.push(output);
        }

        results
    }

    /// Plan a step with automatic fallback on rate-limit errors
    async fn plan_with_fallback(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt_override: Option<&str>,
    ) -> crate::error::Result<crate::planner::PlanResponse> {
        let result = match system_prompt_override {
            Some(p) => self.planner.plan_step_with_system_prompt(messages, tools, p).await,
            None => self.planner.plan_step(messages, tools).await,
        };
        match result {
            Ok(resp) => Ok(resp),
            Err(crate::error::Error::Llm(cratos_llm::Error::RateLimit))
                if self.fallback_planner.is_some() =>
            {
                warn!("Primary provider rate limited, trying fallback");
                let fb = self.fallback_planner.as_ref().unwrap();
                match system_prompt_override {
                    Some(p) => fb.plan_step_with_system_prompt(messages, tools, p).await,
                    None => fb.plan_step(messages, tools).await,
                }
            }
            Err(crate::error::Error::Llm(cratos_llm::Error::ServerError(ref msg)))
                if self.fallback_planner.is_some() =>
            {
                warn!(error = %msg, "Primary provider server error, trying fallback");
                let fb = self.fallback_planner.as_ref().unwrap();
                match system_prompt_override {
                    Some(p) => fb.plan_step_with_system_prompt(messages, tools, p).await,
                    None => fb.plan_step(messages, tools).await,
                }
            }
            // Auth/Permission errors (e.g. expired token, 403 PERMISSION_DENIED)
            Err(crate::error::Error::Llm(cratos_llm::Error::Api(ref msg)))
                if self.fallback_planner.is_some() && is_auth_or_permission_error(msg) =>
            {
                warn!(error = %msg, "Primary provider auth/permission error, trying fallback");
                let fb = self.fallback_planner.as_ref().unwrap();
                match system_prompt_override {
                    Some(p) => fb.plan_step_with_system_prompt(messages, tools, p).await,
                    None => fb.plan_step(messages, tools).await,
                }
            }
            // Network errors
            Err(crate::error::Error::Llm(cratos_llm::Error::Network(ref msg)))
                if self.fallback_planner.is_some() =>
            {
                warn!(error = %msg, "Primary provider network error, trying fallback");
                let fb = self.fallback_planner.as_ref().unwrap();
                match system_prompt_override {
                    Some(p) => fb.plan_step_with_system_prompt(messages, tools, p).await,
                    None => fb.plan_step(messages, tools).await,
                }
            }
            // Timeout errors
            Err(crate::error::Error::Llm(cratos_llm::Error::Timeout(_)))
                if self.fallback_planner.is_some() =>
            {
                warn!("Primary provider timed out, trying fallback");
                let fb = self.fallback_planner.as_ref().unwrap();
                match system_prompt_override {
                    Some(p) => fb.plan_step_with_system_prompt(messages, tools, p).await,
                    None => fb.plan_step(messages, tools).await,
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Route input to a persona via LLM classification.
    /// Returns the persona name (not agent_id).
    /// Falls back to "cratos" on any error — NO keyword matching.
    async fn route_by_llm(&self, input: &str) -> String {
        // Short greetings/interjections → skip LLM call
        if input.split_whitespace().count() < 3 {
            return "cratos".to_string();
        }

        let start = std::time::Instant::now();
        match self.planner.classify(PERSONA_CLASSIFICATION_PROMPT, input).await {
            Ok(raw) => {
                let persona = raw.trim().trim_matches('"').to_lowercase();
                let ms = start.elapsed().as_millis();
                if let Some(mapping) = &self.persona_mapping {
                    if mapping.is_persona(&persona) {
                        debug!(persona = %persona, ms = %ms, "LLM persona classification");
                        return persona;
                    }
                }
                warn!(raw = %raw, ms = %ms, "LLM returned unknown persona, defaulting to cratos");
                "cratos".to_string()
            }
            Err(e) => {
                warn!(error = %e, "LLM classify failed, defaulting to cratos");
                "cratos".to_string()
            }
        }
    }

    /// Publish an event to the event bus (no-op if no bus is set).
    fn emit(&self, event: OrchestratorEvent) {
        if let Some(bus) = &self.event_bus {
            bus.publish(event);
        }
    }

    /// Log an event to the event store
    async fn log_event(
        &self,
        execution_id: Uuid,
        event_type: EventType,
        payload: &serde_json::Value,
    ) {
        if !self.config.enable_logging {
            return;
        }

        if let Some(store) = &self.event_store {
            // Use the Event builder pattern from cratos-replay
            let event = Event::new(execution_id, 0, event_type).with_payload(payload.clone());

            if let Err(e) = store.append(event).await {
                warn!(error = %e, "Failed to log event");
            }
        }
    }

    /// Clear a user's session
    pub async fn clear_session(&self, session_key: &str) {
        if let Ok(Some(mut session)) = self.memory.get(session_key).await {
            session.clear();
            let _ = self.memory.save(&session).await;
        }
    }

    /// Get session message count
    pub async fn session_message_count(&self, session_key: &str) -> usize {
        self.memory
            .get(session_key)
            .await
            .ok()
            .flatten()
            .map(|s| s.message_count())
            .unwrap_or(0)
    }
}

/// Check if an error message indicates an authentication or permission problem.
fn is_auth_or_permission_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("authentication")
        || lower.contains("permission")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("unauthenticated")
}

/// Detect if the model's first response is likely a refusal to use tools.
///
/// Structural detection: on the first iteration, if the model gives a short
/// text-only response without any tool calls, it's almost certainly refusing
/// to act. Genuine knowledge answers are typically longer. This avoids
/// hardcoded keyword lists which are fragile and language-dependent.
fn is_tool_refusal(content: &str) -> bool {
    let trimmed = content.trim();
    // Empty or very short responses (< 200 chars) without tool calls on iteration 1
    // are almost certainly refusals. Genuine conversational answers or knowledge
    // responses are longer.
    trimmed.is_empty() || trimmed.len() < 200
}

/// Sanitize LLM response before sending to users.
///
/// Weak models sometimes generate XML-like tags (e.g. `<tool_response>`) in their text output
/// instead of using the function calling API properly. This strips those artifacts.
fn sanitize_response(text: &str) -> String {
    use regex::Regex;

    // Lazy-init compiled regex patterns
    static PATTERNS: std::sync::OnceLock<Vec<Regex>> = std::sync::OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            // <tool_response>...</tool_response> and similar tags
            Regex::new(r"(?s)</?(?:tool_response|tool_call|function_call|function_response|system|thinking)>").unwrap(),
            // JSON blocks that look like raw tool output: {"key": ...}
            // Only strip if preceded by a tag-like marker
            Regex::new(r"(?s)<tool_response>\s*\{[^}]*\}\s*</tool_response>").unwrap(),
        ]
    });

    let mut result = text.to_string();
    for pat in patterns {
        result = pat.replace_all(&result, "").to_string();
    }

    // Clean up excessive blank lines left behind
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_input() {
        let input =
            OrchestratorInput::new("telegram", "123", "456", "Hello").with_thread("thread_1");

        assert_eq!(input.channel_type, "telegram");
        assert_eq!(input.session_key(), "telegram:123:456");
        assert_eq!(input.thread_id, Some("thread_1".to_string()));
    }

    #[test]
    fn test_orchestrator_config() {
        let config = OrchestratorConfig::new()
            .with_max_iterations(5)
            .with_logging(false);

        assert_eq!(config.max_iterations, 5);
        assert!(!config.enable_logging);
    }

    #[test]
    fn test_execution_status() {
        assert_eq!(
            serde_json::to_string(&ExecutionStatus::Completed).unwrap(),
            "\"completed\""
        );
    }
}
