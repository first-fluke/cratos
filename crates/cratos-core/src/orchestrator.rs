//! Orchestrator - Main execution loop
//!
//! This module provides the main orchestration logic that ties together
//! the planner, tools, memory, and replay systems.

use crate::error::Result;
use crate::memory::{MemoryStore, SessionContext, SessionStore, WorkingMemory};
use crate::planner::{Planner, PlannerConfig};
use cratos_llm::{LlmProvider, ToolCall};
use cratos_replay::{Event, EventStoreTrait, EventType};
use cratos_tools::{RunnerConfig, ToolRegistry, ToolRunner};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

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
    /// Total iterations
    pub iterations: usize,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Model used
    pub model: Option<String>,
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
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            enable_logging: true,
            planner_config: PlannerConfig::default(),
            runner_config: RunnerConfig::default(),
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
            config,
        }
    }

    /// Set the event store for logging
    pub fn with_event_store(mut self, store: Arc<dyn EventStoreTrait>) -> Self {
        self.event_store = Some(store);
        self
    }

    /// Set the memory store
    pub fn with_memory(mut self, memory: Arc<dyn SessionStore>) -> Self {
        self.memory = memory;
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
            let mut session = self
                .memory
                .get(&session_key)
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| crate::memory::SessionContext::new(&session_key));

            session.add_user_message(&input.text);

            // Save updated session
            let _ = self.memory.save(&session).await;
            session.get_messages().to_vec()
        };

        // Create working memory
        let mut working_memory = WorkingMemory::with_execution_id(execution_id);
        let mut tool_call_records = Vec::new();
        let mut final_response = String::new();
        let mut model_used = None;
        let mut iteration = 0;

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

            debug!(
                execution_id = %execution_id,
                iteration = %iteration,
                "Planning step"
            );

            // Plan the next step
            let plan_response = match self.planner.plan_step(&messages, &tools).await {
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

                    return Ok(ExecutionResult {
                        execution_id,
                        status: ExecutionStatus::Failed,
                        response: format!("I encountered an error: {}", e),
                        tool_calls: tool_call_records,
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
                if let Some(content) = plan_response.content {
                    final_response = content;
                }
                break;
            }

            // Execute tool calls
            if !plan_response.tool_calls.is_empty() {
                let results = self
                    .execute_tool_calls(
                        execution_id,
                        &plan_response.tool_calls,
                        &mut working_memory,
                        &mut tool_call_records,
                    )
                    .await;

                // Build tool result messages
                let tool_messages =
                    Planner::build_tool_result_messages(&plan_response.tool_calls, &results);

                // Add tool messages to session
                if let Ok(Some(mut session)) = self.memory.get(&session_key).await {
                    for msg in tool_messages {
                        if let Some(tool_call_id) = &msg.tool_call_id {
                            session.add_tool_message(&msg.content, tool_call_id);
                        }
                    }
                    let _ = self.memory.save(&session).await;
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

        // Update session with assistant response
        if let Ok(Some(mut session)) = self.memory.get(&session_key).await {
            session.add_assistant_message(&final_response);
            let _ = self.memory.save(&session).await;
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        info!(
            execution_id = %execution_id,
            iterations = %iteration,
            duration_ms = %duration_ms,
            "Execution completed"
        );

        Ok(ExecutionResult {
            execution_id,
            status: ExecutionStatus::Completed,
            response: final_response,
            tool_calls: tool_call_records,
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
            debug!(
                execution_id = %execution_id,
                tool = %call.name,
                "Executing tool"
            );

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

            // Parse arguments
            let input: serde_json::Value =
                serde_json::from_str(&call.arguments).unwrap_or_else(|_| serde_json::json!({}));

            let start = std::time::Instant::now();
            let result = self.runner.execute(&call.name, input.clone()).await;
            let duration_ms = start.elapsed().as_millis() as u64;

            let (output, success, error) = match result {
                Ok(exec_result) => {
                    let output = exec_result.result.output.clone();
                    (output, exec_result.result.success, exec_result.result.error)
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
