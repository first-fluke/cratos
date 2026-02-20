use super::parsing::AgentTaskParser;
use super::types::{
    AgentResponse, ExecutionContext, OrchestratorError, OrchestratorResult, ParsedAgentTask,
    TaskStatus,
};
use super::AgentOrchestrator;
use std::time::Instant;
use tracing::{debug, info, warn};

impl AgentOrchestrator {
    /// Handle a single input (may contain multiple @mentions)
    pub async fn handle(
        &self,
        input: &str,
        context: &ExecutionContext,
    ) -> OrchestratorResult<Vec<AgentResponse>> {
        // Check cancellation before starting
        if self.is_cancelled() {
            return Err(OrchestratorError::Cancelled);
        }

        // Check recursion depth
        self.enter_depth()?;

        let result = self.handle_inner(input, context).await;

        self.exit_depth();
        result
    }

    /// Inner handle implementation
    async fn handle_inner(
        &self,
        input: &str,
        context: &ExecutionContext,
    ) -> OrchestratorResult<Vec<AgentResponse>> {
        let parser = AgentTaskParser::new(&self.agents, self.config.default_agent.clone());
        let tasks = parser.parse_input(input)?;

        if tasks.is_empty() {
            return Err(OrchestratorError::NoAgentMatched);
        }

        // Execute tasks (parallel if multiple)
        if tasks.len() == 1 {
            let response = self.execute_task(&tasks[0], context).await?;
            Ok(vec![response])
        } else {
            self.execute_parallel(tasks, context).await
        }
    }

    /// Execute a single task
    pub(super) async fn execute_task(
        &self,
        task: &ParsedAgentTask,
        context: &ExecutionContext,
    ) -> OrchestratorResult<AgentResponse> {
        // Check cancellation before starting
        if self.is_cancelled() {
            let mut tasks = self.active_tasks.write().await;
            tasks.insert(task.agent_id.clone(), TaskStatus::Cancelled);
            return Err(OrchestratorError::Cancelled);
        }

        let start = Instant::now();

        let agent = self
            .agents
            .get(&task.agent_id)
            .ok_or_else(|| OrchestratorError::AgentNotFound(task.agent_id.clone()))?;

        debug!(
            agent_id = %task.agent_id,
            provider = %agent.cli.provider,
            "Executing agent task"
        );

        // Update task status
        {
            let mut tasks = self.active_tasks.write().await;
            tasks.insert(task.agent_id.clone(), TaskStatus::Running);
        }

        // Get CLI provider
        let provider = self.cli_registry.get(&agent.cli.provider).ok_or_else(|| {
            OrchestratorError::Configuration(format!(
                "CLI provider '{}' not found",
                agent.cli.provider
            ))
        })?;

        // Determine workspace
        let workspace = context.workspace.clone().unwrap_or_else(|| {
            self.config
                .workspace_base
                .join(&context.session_id)
                .join(&task.agent_id)
        });

        // Execute via CLI provider with cancellation support
        let cancel_token = self.cancel_token.child_token();
        let execution = provider.execute(&task.prompt, &agent.persona.prompt, Some(&workspace));

        let result = tokio::select! {
            result = execution => result,
            _ = cancel_token.cancelled() => {
                warn!(agent_id = %task.agent_id, "Task cancelled by user");
                let mut tasks = self.active_tasks.write().await;
                tasks.insert(task.agent_id.clone(), TaskStatus::Cancelled);
                return Err(OrchestratorError::Cancelled);
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        let response = match result {
            Ok(content) => {
                // Estimate tokens used (rough estimate: 4 chars per token)
                let estimated_tokens = (content.len() / 4) as u64;
                if let Err(e) = self.track_tokens(estimated_tokens) {
                    warn!(agent_id = %task.agent_id, "Token budget exceeded");
                    let mut tasks = self.active_tasks.write().await;
                    tasks.insert(task.agent_id.clone(), TaskStatus::Failed(e.to_string()));
                    return Err(e);
                }

                info!(
                    agent_id = %task.agent_id,
                    duration_ms = duration_ms,
                    tokens = estimated_tokens,
                    "Agent task completed"
                );
                AgentResponse {
                    agent_id: task.agent_id.clone(),
                    content,
                    success: true,
                    error: None,
                    duration_ms,
                }
            }
            Err(e) => {
                warn!(
                    agent_id = %task.agent_id,
                    error = %e,
                    "Agent task failed"
                );
                AgentResponse {
                    agent_id: task.agent_id.clone(),
                    content: String::new(),
                    success: false,
                    error: Some(e.to_string()),
                    duration_ms,
                }
            }
        };

        // Update task status
        {
            let mut tasks = self.active_tasks.write().await;
            if response.success {
                tasks.insert(
                    task.agent_id.clone(),
                    TaskStatus::Completed(response.clone()),
                );
            } else {
                tasks.insert(
                    task.agent_id.clone(),
                    TaskStatus::Failed(response.error.clone().unwrap_or_default()),
                );
            }
        }

        Ok(response)
    }

    /// Execute multiple tasks in parallel
    pub(super) async fn execute_parallel(
        &self,
        tasks: Vec<ParsedAgentTask>,
        context: &ExecutionContext,
    ) -> OrchestratorResult<Vec<AgentResponse>> {
        // Check cancellation before starting
        if self.is_cancelled() {
            return Err(OrchestratorError::Cancelled);
        }

        let max_parallel = self.config.max_parallel.min(tasks.len());

        info!(
            task_count = tasks.len(),
            max_parallel = max_parallel,
            "Executing tasks in parallel"
        );

        // Create futures for all tasks
        let futures: Vec<_> = tasks
            .into_iter()
            .map(|task| {
                let ctx = context.clone();
                async move { self.execute_task(&task, &ctx).await }
            })
            .collect();

        // Execute all with cancellation support
        let cancel_token = self.cancel_token.child_token();
        let all_futures = futures::future::join_all(futures);

        let results = tokio::select! {
            results = all_futures => results,
            _ = cancel_token.cancelled() => {
                warn!("Parallel execution cancelled by user");
                return Err(OrchestratorError::Cancelled);
            }
        };

        // Collect results (skip cancelled tasks)
        let mut responses = Vec::new();
        for result in results {
            match result {
                Ok(response) => responses.push(response),
                Err(OrchestratorError::Cancelled) => {
                    continue;
                }
                Err(e) => {
                    warn!(error = %e, "Parallel task failed");
                }
            }
        }

        Ok(responses)
    }
}
