//! Multi-persona execution
//!
//! Contains multi-persona execution logic for the Orchestrator:
//! - Parallel execution
//! - Pipeline execution
//! - Collaborative execution

use crate::agents::{ExecutionMode, MultiPersonaExtraction, PersonaMention};
use crate::error::Result;
use cratos_llm::Message;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

use super::config::OrchestratorInput;
use super::core::Orchestrator;
use super::types::{ExecutionResult, ExecutionStatus, ToolCallRecord};

/// Result from a single persona execution (internal)
#[derive(Debug)]
pub(crate) struct PersonaExecutionResult {
    pub persona_name: String,
    pub response: String,
    pub tool_calls: Vec<ToolCallRecord>,
    pub success: bool,
    #[allow(dead_code)] // Reserved for future metrics/logging
    pub duration_ms: u64,
    pub model: Option<String>,
}

impl Orchestrator {
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Multi-Persona Execution
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Execute multiple personas based on extraction mode
    pub(crate) async fn execute_multi_persona(
        &self,
        execution_id: Uuid,
        extraction: MultiPersonaExtraction,
        input: &OrchestratorInput,
        messages: &[Message],
        cancel_token: &CancellationToken,
    ) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();

        let result = match extraction.mode {
            ExecutionMode::Parallel => {
                self.execute_parallel_personas(
                    execution_id,
                    extraction,
                    input,
                    messages,
                    cancel_token,
                )
                .await
            }
            ExecutionMode::Pipeline => {
                self.execute_pipeline_personas(
                    execution_id,
                    extraction,
                    input,
                    messages,
                    cancel_token,
                )
                .await
            }
            ExecutionMode::Collaborative => {
                self.execute_collaborative_personas(
                    execution_id,
                    extraction,
                    input,
                    messages,
                    cancel_token,
                )
                .await
            }
        };

        // Log duration for monitoring
        info!(
            execution_id = %execution_id,
            duration_ms = start.elapsed().as_millis() as u64,
            "Multi-persona execution completed"
        );

        result
    }

    /// Execute personas in parallel and aggregate results
    async fn execute_parallel_personas(
        &self,
        execution_id: Uuid,
        extraction: MultiPersonaExtraction,
        input: &OrchestratorInput,
        messages: &[Message],
        cancel_token: &CancellationToken,
    ) -> Result<ExecutionResult> {
        use futures::future::join_all;

        let start = std::time::Instant::now();
        let persona_names: Vec<String> =
            extraction.personas.iter().map(|p| p.name.clone()).collect();

        info!(
            execution_id = %execution_id,
            personas = ?persona_names,
            task = %extraction.rest,
            "Starting parallel persona execution"
        );

        // Build futures for each persona
        let futures: Vec<_> = extraction
            .personas
            .iter()
            .map(|persona| {
                let persona = persona.clone();
                let task = extraction.rest.clone();
                let input = input.clone();
                let messages = messages.to_vec();
                let cancel = cancel_token.clone();

                async move {
                    self.run_single_persona(
                        execution_id,
                        &persona,
                        &task,
                        &input,
                        &messages,
                        &cancel,
                    )
                    .await
                }
            })
            .collect();

        // Execute with cancellation support
        let results = tokio::select! {
            r = join_all(futures) => r,
            _ = cancel_token.cancelled() => {
                return Ok(ExecutionResult {
                    execution_id,
                    status: ExecutionStatus::Cancelled,
                    response: "Multi-persona execution cancelled".to_string(),
                    tool_calls: vec![],
                    artifacts: vec![],
                    iterations: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                    model: None,
                });
            }
        };

        // Aggregate results
        self.aggregate_persona_results(execution_id, &persona_names, results, start)
    }

    /// Run a single persona with its own system prompt
    pub(crate) async fn run_single_persona(
        &self,
        _execution_id: Uuid,
        persona: &PersonaMention,
        task: &str,
        input: &OrchestratorInput,
        base_messages: &[Message],
        cancel_token: &CancellationToken,
    ) -> PersonaExecutionResult {
        let persona_start = std::time::Instant::now();
        let persona_name = persona.name.clone();

        info!(persona = %persona_name, task = %task, "Running persona");

        // Build persona-specific system prompt
        let system_prompt = self
            .persona_mapping
            .as_ref()
            .and_then(|m| m.get_system_prompt(&persona.name, &input.user_id))
            .map(|p| {
                format!(
                    "{}\n\n---\n## Active Persona\n{}",
                    self.planner.config().system_prompt,
                    p
                )
            });

        // Build messages with task
        let mut messages = base_messages.to_vec();
        // Replace or append the user message with the extracted task
        if let Some(last) = messages.last_mut() {
            if last.role == cratos_llm::MessageRole::User {
                last.content = task.to_string();
            }
        }

        // Get available tools
        let tools = self.runner.registry().to_llm_tools();

        // Single LLM call (simplified - no tool loop for now)
        let plan_result = match &system_prompt {
            Some(sp) => {
                self.planner
                    .plan_step_with_system_prompt(&messages, &tools, sp, None)
                    .await
            }
            None => self.planner.plan_step(&messages, &tools, None).await,
        };

        match plan_result {
            Ok(plan) => {
                let response = plan.content.clone().unwrap_or_default();
                let mut tool_calls = Vec::new();

                // Execute tool calls if any
                for tc in &plan.tool_calls {
                    if cancel_token.is_cancelled() {
                        break;
                    }

                    // Parse arguments from JSON string to Value
                    let args: serde_json::Value =
                        serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);

                    let tool_start = std::time::Instant::now();
                    let result = self.runner.execute(&tc.name, args.clone()).await;

                    let (output, success) = match result {
                        Ok(exec_result) => (exec_result.result.output, exec_result.result.success),
                        Err(e) => (serde_json::Value::String(e.to_string()), false),
                    };

                    tool_calls.push(ToolCallRecord {
                        tool_name: tc.name.clone(),
                        input: args,
                        output,
                        success,
                        duration_ms: tool_start.elapsed().as_millis() as u64,
                        persona_name: Some(persona_name.clone()),
                    });
                }

                PersonaExecutionResult {
                    persona_name,
                    response,
                    tool_calls,
                    success: true,
                    duration_ms: persona_start.elapsed().as_millis() as u64,
                    model: Some(plan.model),
                }
            }
            Err(e) => {
                error!(persona = %persona_name, error = %e, "Persona execution failed");
                PersonaExecutionResult {
                    persona_name,
                    response: format!("Error: {}", e),
                    tool_calls: vec![],
                    success: false,
                    duration_ms: persona_start.elapsed().as_millis() as u64,
                    model: None,
                }
            }
        }
    }

    /// Aggregate results from multiple persona executions
    fn aggregate_persona_results(
        &self,
        execution_id: Uuid,
        persona_names: &[String],
        results: Vec<PersonaExecutionResult>,
        start: std::time::Instant,
    ) -> Result<ExecutionResult> {
        let all_success = results.iter().all(|r| r.success);
        let any_success = results.iter().any(|r| r.success);

        // Build combined response with persona sections
        let response = results
            .iter()
            .map(|r| {
                let status_emoji = if r.success { "✓" } else { "✗" };
                format!("## {} {}\n{}", r.persona_name, status_emoji, r.response)
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        // Combine all tool calls
        let tool_calls: Vec<ToolCallRecord> =
            results.iter().flat_map(|r| r.tool_calls.clone()).collect();

        // Determine status
        let status = if all_success {
            ExecutionStatus::Completed
        } else if any_success {
            ExecutionStatus::PartialSuccess
        } else {
            ExecutionStatus::Failed
        };

        // Use the first successful model or None
        let model = results.iter().find_map(|r| r.model.clone());

        info!(
            execution_id = %execution_id,
            personas = ?persona_names,
            status = ?status,
            total_tool_calls = tool_calls.len(),
            "Multi-persona aggregation complete"
        );

        Ok(ExecutionResult {
            execution_id,
            status,
            response,
            tool_calls,
            artifacts: vec![],
            iterations: results.len(),
            duration_ms: start.elapsed().as_millis() as u64,
            model,
        })
    }

    /// Execute personas in sequence (Pipeline mode)
    ///
    /// Each persona's output becomes context for the next persona.
    /// Example: `@athena 계획 -> @sindri 구현` runs Athena first, then passes
    /// Athena's output to Sindri as context.
    async fn execute_pipeline_personas(
        &self,
        execution_id: Uuid,
        extraction: MultiPersonaExtraction,
        input: &OrchestratorInput,
        messages: &[Message],
        cancel_token: &CancellationToken,
    ) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();
        let persona_names: Vec<String> =
            extraction.personas.iter().map(|p| p.name.clone()).collect();

        info!(
            execution_id = %execution_id,
            personas = ?persona_names,
            "Starting pipeline persona execution"
        );

        let mut results: Vec<PersonaExecutionResult> = Vec::new();
        let mut accumulated_context = String::new();

        for (idx, persona) in extraction.personas.iter().enumerate() {
            if cancel_token.is_cancelled() {
                warn!("Pipeline cancelled at stage {}", idx);
                break;
            }

            // Build task: persona's instruction + accumulated context from previous stages
            let stage_task = if let Some(ref instruction) = persona.instruction {
                if accumulated_context.is_empty() {
                    instruction.clone()
                } else {
                    format!(
                        "{}\n\n---\n## Previous Stage Results\n{}",
                        instruction, accumulated_context
                    )
                }
            } else if accumulated_context.is_empty() {
                extraction.rest.clone()
            } else {
                format!(
                    "{}\n\n---\n## Previous Stage Results\n{}",
                    extraction.rest, accumulated_context
                )
            };

            info!(
                stage = idx + 1,
                persona = %persona.name,
                "Executing pipeline stage"
            );

            let result = self
                .run_single_persona(
                    execution_id,
                    persona,
                    &stage_task,
                    input,
                    messages,
                    cancel_token,
                )
                .await;

            // Accumulate successful output for next stage
            if result.success {
                if !accumulated_context.is_empty() {
                    accumulated_context.push_str("\n\n");
                }
                accumulated_context.push_str(&format!(
                    "### {} output:\n{}",
                    persona.name, result.response
                ));
            }

            results.push(result);
        }

        // Aggregate with pipeline-specific formatting
        self.aggregate_pipeline_results(execution_id, &persona_names, results, start)
    }

    /// Aggregate results from pipeline execution
    fn aggregate_pipeline_results(
        &self,
        execution_id: Uuid,
        persona_names: &[String],
        results: Vec<PersonaExecutionResult>,
        start: std::time::Instant,
    ) -> Result<ExecutionResult> {
        let all_success = results.iter().all(|r| r.success);
        let any_success = results.iter().any(|r| r.success);

        // Build response showing pipeline flow
        let response = results
            .iter()
            .enumerate()
            .map(|(idx, r)| {
                let status_emoji = if r.success { "✓" } else { "✗" };
                format!(
                    "## Stage {} - {} {}\n{}",
                    idx + 1,
                    r.persona_name,
                    status_emoji,
                    r.response
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n→\n\n");

        let tool_calls: Vec<ToolCallRecord> =
            results.iter().flat_map(|r| r.tool_calls.clone()).collect();

        let status = if all_success {
            ExecutionStatus::Completed
        } else if any_success {
            ExecutionStatus::PartialSuccess
        } else {
            ExecutionStatus::Failed
        };

        let model = results.iter().find_map(|r| r.model.clone());

        info!(
            execution_id = %execution_id,
            personas = ?persona_names,
            status = ?status,
            stages = results.len(),
            "Pipeline execution complete"
        );

        Ok(ExecutionResult {
            execution_id,
            status,
            response,
            tool_calls,
            artifacts: vec![],
            iterations: results.len(),
            duration_ms: start.elapsed().as_millis() as u64,
            model,
        })
    }

    /// Execute personas collaboratively (Collaborative mode)
    ///
    /// Personas work on the same task and can see each other's responses.
    /// Multiple rounds of collaboration until consensus or max rounds.
    async fn execute_collaborative_personas(
        &self,
        execution_id: Uuid,
        extraction: MultiPersonaExtraction,
        input: &OrchestratorInput,
        messages: &[Message],
        cancel_token: &CancellationToken,
    ) -> Result<ExecutionResult> {
        use futures::future::join_all;

        let start = std::time::Instant::now();
        let persona_names: Vec<String> =
            extraction.personas.iter().map(|p| p.name.clone()).collect();
        const MAX_ROUNDS: usize = 2; // Limit collaboration rounds

        info!(
            execution_id = %execution_id,
            personas = ?persona_names,
            task = %extraction.rest,
            "Starting collaborative persona execution"
        );

        let mut all_results: Vec<PersonaExecutionResult> = Vec::new();
        let mut collaboration_context = String::new();

        for round in 0..MAX_ROUNDS {
            if cancel_token.is_cancelled() {
                warn!("Collaboration cancelled at round {}", round + 1);
                break;
            }

            info!(round = round + 1, "Collaboration round");

            // Build task with collaboration context
            let round_task = if collaboration_context.is_empty() {
                extraction.rest.clone()
            } else {
                format!(
                    "{}\n\n---\n## Collaborators' Previous Responses\n{}\n\n**Build on or refine the above responses.**",
                    extraction.rest, collaboration_context
                )
            };

            // Execute all personas in parallel for this round
            let futures: Vec<_> = extraction
                .personas
                .iter()
                .map(|persona| {
                    let persona = persona.clone();
                    let task = round_task.clone();
                    let input = input.clone();
                    let messages = messages.to_vec();
                    let cancel = cancel_token.clone();

                    async move {
                        self.run_single_persona(
                            execution_id,
                            &persona,
                            &task,
                            &input,
                            &messages,
                            &cancel,
                        )
                        .await
                    }
                })
                .collect();

            let round_results = tokio::select! {
                r = join_all(futures) => r,
                _ = cancel_token.cancelled() => {
                    break;
                }
            };

            // Update collaboration context with this round's responses
            collaboration_context = round_results
                .iter()
                .filter(|r| r.success)
                .map(|r| format!("### {} says:\n{}", r.persona_name, r.response))
                .collect::<Vec<_>>()
                .join("\n\n");

            all_results.extend(round_results);

            // Check for early termination: if responses are converging (similar), stop
            // For now, always run MAX_ROUNDS for simplicity
        }

        // Aggregate collaborative results
        self.aggregate_collaborative_results(
            execution_id,
            &persona_names,
            all_results,
            MAX_ROUNDS,
            start,
        )
    }

    /// Aggregate results from collaborative execution
    fn aggregate_collaborative_results(
        &self,
        execution_id: Uuid,
        persona_names: &[String],
        results: Vec<PersonaExecutionResult>,
        rounds: usize,
        start: std::time::Instant,
    ) -> Result<ExecutionResult> {
        let all_success = results.iter().all(|r| r.success);
        let any_success = results.iter().any(|r| r.success);

        // Group results by round
        let personas_per_round = persona_names.len();
        let mut response_parts: Vec<String> = Vec::new();

        for round in 0..rounds {
            let round_start = round * personas_per_round;
            let round_end = (round_start + personas_per_round).min(results.len());

            if round_start >= results.len() {
                break;
            }

            let round_responses: Vec<String> = results[round_start..round_end]
                .iter()
                .map(|r| {
                    let status_emoji = if r.success { "✓" } else { "✗" };
                    format!("### {} {}\n{}", r.persona_name, status_emoji, r.response)
                })
                .collect();

            response_parts.push(format!(
                "## Collaboration Round {}\n{}",
                round + 1,
                round_responses.join("\n\n")
            ));
        }

        let response = response_parts.join("\n\n---\n\n");

        let tool_calls: Vec<ToolCallRecord> =
            results.iter().flat_map(|r| r.tool_calls.clone()).collect();

        let status = if all_success {
            ExecutionStatus::Completed
        } else if any_success {
            ExecutionStatus::PartialSuccess
        } else {
            ExecutionStatus::Failed
        };

        let model = results.iter().find_map(|r| r.model.clone());

        info!(
            execution_id = %execution_id,
            personas = ?persona_names,
            status = ?status,
            rounds = rounds,
            total_responses = results.len(),
            "Collaborative execution complete"
        );

        Ok(ExecutionResult {
            execution_id,
            status,
            response,
            tool_calls,
            artifacts: vec![],
            iterations: results.len(),
            duration_ms: start.elapsed().as_millis() as u64,
            model,
        })
    }
}
