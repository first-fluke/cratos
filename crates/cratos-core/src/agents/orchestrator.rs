//! Agent Orchestrator
//!
//! Unified multi-agent orchestration system for Cratos.
//!
//! ## Usage
//!
//! ```text
//! @backend implement the API
//! @frontend create the UI
//! @backend API @frontend UI parallel execution
//! ```
//!
//! ## Safety Features
//!
//! - **CancellationToken**: User can cancel running tasks at any time
//! - **Token Budget**: Limits total tokens per session to prevent runaway costs
//! - **Timeout**: Each task has a configurable timeout

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Pre-compiled regex for @mention parsing (e.g., "@backend do something")
static MENTION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@(\w+)\s+").expect("MENTION_REGEX is a compile-time constant"));

/// Default token budget per session (100K tokens)
const DEFAULT_TOKEN_BUDGET: u64 = 100_000;

/// Default max recursion depth for agent calls
const DEFAULT_MAX_DEPTH: u32 = 3;

use super::cli_registry::{CliError, CliRegistry};
use super::config::AgentConfig;

/// Orchestrator errors
#[derive(Debug, Error)]
pub enum OrchestratorError {
    /// Agent not found
    #[error("Agent '{0}' not found")]
    AgentNotFound(String),

    /// No agent matched
    #[error("No agent matched for input")]
    NoAgentMatched,

    /// CLI execution failed
    #[error("CLI error: {0}")]
    CliError(#[from] CliError),

    /// Parse error
    #[error("Failed to parse input: {0}")]
    ParseError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Task was cancelled by user
    #[error("Task cancelled by user")]
    Cancelled,

    /// Token budget exceeded
    #[error("Token budget exceeded: used {used} of {budget} tokens")]
    BudgetExceeded {
        /// Tokens used so far
        used: u64,
        /// Token budget limit
        budget: u64,
    },

    /// Max recursion depth exceeded
    #[error("Max recursion depth exceeded: {0}")]
    MaxDepthExceeded(u32),
}

/// Orchestrator result type
pub type OrchestratorResult<T> = std::result::Result<T, OrchestratorError>;

/// Orchestrator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Default agent for unmatched inputs
    #[serde(default = "default_agent")]
    pub default_agent: String,
    /// Enable semantic routing (keyword matching)
    #[serde(default = "default_true")]
    pub semantic_routing: bool,
    /// Workspace base directory
    #[serde(default = "default_workspace")]
    pub workspace_base: PathBuf,
    /// Max parallel agents
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,
    /// Token budget per session (0 = unlimited)
    #[serde(default = "default_token_budget")]
    pub token_budget: u64,
    /// Max recursion depth for agent-to-agent calls
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
}

fn default_agent() -> String {
    "researcher".to_string()
}

fn default_true() -> bool {
    true
}

fn default_workspace() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cratos")
        .join("workspaces")
}

fn default_max_parallel() -> usize {
    5
}

fn default_token_budget() -> u64 {
    DEFAULT_TOKEN_BUDGET
}

fn default_max_depth() -> u32 {
    DEFAULT_MAX_DEPTH
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            default_agent: default_agent(),
            semantic_routing: true,
            workspace_base: default_workspace(),
            max_parallel: default_max_parallel(),
            token_budget: default_token_budget(),
            max_depth: default_max_depth(),
        }
    }
}

/// Parsed agent task from user input
#[derive(Debug, Clone)]
pub struct ParsedAgentTask {
    /// Agent ID
    pub agent_id: String,
    /// Task prompt
    pub prompt: String,
    /// Whether this was an explicit mention
    pub explicit_mention: bool,
}

/// Agent response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// Agent ID that responded
    pub agent_id: String,
    /// Response content
    pub content: String,
    /// Whether execution was successful
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

/// Task execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Pending execution
    Pending,
    /// Currently running
    Running,
    /// Completed successfully
    Completed(AgentResponse),
    /// Failed with error
    Failed(String),
    /// Cancelled by user
    Cancelled,
}

/// Session context for agent execution
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Session ID
    pub session_id: String,
    /// Channel (telegram, slack, cli, etc.)
    pub channel: String,
    /// User ID (optional)
    pub user_id: Option<String>,
    /// Custom workspace path (optional)
    pub workspace: Option<PathBuf>,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            channel: "cli".to_string(),
            user_id: None,
            workspace: None,
        }
    }
}

/// Agent Orchestrator
///
/// Manages multi-agent execution with:
/// - Explicit @mention routing
/// - Semantic keyword routing
/// - CLI provider mapping
/// - Parallel execution support
/// - Cancellation support
/// - Token budget tracking
pub struct AgentOrchestrator {
    /// Registered agents
    agents: HashMap<String, AgentConfig>,
    /// CLI provider registry
    cli_registry: Arc<CliRegistry>,
    /// Configuration
    config: OrchestratorConfig,
    /// Active tasks
    active_tasks: RwLock<HashMap<String, TaskStatus>>,
    /// Cancellation token for stopping all tasks
    cancel_token: CancellationToken,
    /// Total tokens used in this session
    tokens_used: AtomicU64,
    /// Current recursion depth
    current_depth: AtomicU64,
}

impl AgentOrchestrator {
    /// Create a new orchestrator with default agents
    pub fn new(config: OrchestratorConfig) -> Self {
        let mut agents = HashMap::new();
        for agent in AgentConfig::defaults() {
            agents.insert(agent.id.clone(), agent);
        }

        Self {
            agents,
            cli_registry: Arc::new(CliRegistry::with_defaults()),
            config,
            active_tasks: RwLock::new(HashMap::new()),
            cancel_token: CancellationToken::new(),
            tokens_used: AtomicU64::new(0),
            current_depth: AtomicU64::new(0),
        }
    }

    /// Create with custom CLI registry
    pub fn with_cli_registry(config: OrchestratorConfig, cli_registry: CliRegistry) -> Self {
        let mut orchestrator = Self::new(config);
        orchestrator.cli_registry = Arc::new(cli_registry);
        orchestrator
    }

    /// Get a child cancellation token for this orchestrator
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.child_token()
    }

    /// Cancel all running tasks
    pub fn cancel(&self) {
        info!("Cancelling all orchestrator tasks");
        self.cancel_token.cancel();
    }

    /// Check if cancellation was requested
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// Reset the orchestrator for a new session
    pub fn reset(&self) {
        self.tokens_used.store(0, Ordering::SeqCst);
        self.current_depth.store(0, Ordering::SeqCst);
    }

    /// Get current token usage
    pub fn tokens_used(&self) -> u64 {
        self.tokens_used.load(Ordering::SeqCst)
    }

    /// Get remaining token budget
    pub fn tokens_remaining(&self) -> u64 {
        if self.config.token_budget == 0 {
            return u64::MAX; // Unlimited
        }
        self.config
            .token_budget
            .saturating_sub(self.tokens_used.load(Ordering::SeqCst))
    }

    /// Add tokens to usage counter, returns error if budget exceeded
    fn track_tokens(&self, tokens: u64) -> OrchestratorResult<()> {
        if self.config.token_budget == 0 {
            return Ok(()); // Unlimited
        }

        let new_total = self.tokens_used.fetch_add(tokens, Ordering::SeqCst) + tokens;
        if new_total > self.config.token_budget {
            warn!(
                used = new_total,
                budget = self.config.token_budget,
                "Token budget exceeded"
            );
            return Err(OrchestratorError::BudgetExceeded {
                used: new_total,
                budget: self.config.token_budget,
            });
        }
        debug!(tokens = tokens, total = new_total, "Tokens tracked");
        Ok(())
    }

    /// Check and increment recursion depth
    fn enter_depth(&self) -> OrchestratorResult<()> {
        let depth = self.current_depth.fetch_add(1, Ordering::SeqCst) + 1;
        if depth > self.config.max_depth as u64 {
            self.current_depth.fetch_sub(1, Ordering::SeqCst);
            return Err(OrchestratorError::MaxDepthExceeded(self.config.max_depth));
        }
        Ok(())
    }

    /// Decrement recursion depth
    fn exit_depth(&self) {
        self.current_depth.fetch_sub(1, Ordering::SeqCst);
    }

    /// Register an agent
    pub fn register_agent(&mut self, agent: AgentConfig) {
        info!(agent_id = %agent.id, "Registering agent");
        self.agents.insert(agent.id.clone(), agent);
    }

    /// Get an agent by ID
    pub fn get_agent(&self, id: &str) -> Option<&AgentConfig> {
        self.agents.get(id)
    }

    /// List all agent IDs
    pub fn list_agents(&self) -> Vec<&str> {
        self.agents.keys().map(|s| s.as_str()).collect()
    }

    /// List enabled agents
    pub fn list_enabled_agents(&self) -> Vec<&AgentConfig> {
        self.agents.values().filter(|a| a.enabled).collect()
    }

    /// Handle a single input (may contain multiple @mentions)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let response = orchestrator.handle("@backend implement the API", &context).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Cancelled` if the task was cancelled by user.
    /// Returns `BudgetExceeded` if the token budget was exceeded.
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
        let tasks = self.parse_input(input)?;

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

    /// Parse input into agent tasks
    ///
    /// Supports:
    /// - Explicit @mentions: "@backend implement API"
    /// - Multiple mentions: "@backend API @frontend UI"
    /// - Semantic routing (if no mention): "implement API" → backend agent
    pub fn parse_input(&self, input: &str) -> OrchestratorResult<Vec<ParsedAgentTask>> {
        let mut tasks = Vec::new();

        // Find all @mentions
        let mentions: Vec<_> = MENTION_REGEX.find_iter(input).collect();

        if mentions.is_empty() {
            // No explicit mention - use semantic routing
            if self.config.semantic_routing {
                if let Some(agent_id) = self.route_semantically(input) {
                    tasks.push(ParsedAgentTask {
                        agent_id,
                        prompt: input.to_string(),
                        explicit_mention: false,
                    });
                } else {
                    // Use default agent
                    tasks.push(ParsedAgentTask {
                        agent_id: self.config.default_agent.clone(),
                        prompt: input.to_string(),
                        explicit_mention: false,
                    });
                }
            } else {
                // Use default agent
                tasks.push(ParsedAgentTask {
                    agent_id: self.config.default_agent.clone(),
                    prompt: input.to_string(),
                    explicit_mention: false,
                });
            }
        } else if mentions.len() == 1 {
            // Single mention
            let mention = &mentions[0];
            let agent_id = mention.as_str().trim_start_matches('@').trim().to_string();
            let prompt = input[mention.end()..].trim().to_string();

            // Verify agent exists
            if !self.agents.contains_key(&agent_id) {
                return Err(OrchestratorError::AgentNotFound(agent_id));
            }

            tasks.push(ParsedAgentTask {
                agent_id,
                prompt,
                explicit_mention: true,
            });
        } else {
            // Multiple mentions - split into tasks
            for (i, mention) in mentions.iter().enumerate() {
                let agent_id = mention.as_str().trim_start_matches('@').trim().to_string();

                // Verify agent exists
                if !self.agents.contains_key(&agent_id) {
                    return Err(OrchestratorError::AgentNotFound(agent_id));
                }

                // Get prompt until next mention or end
                let start = mention.end();
                let end = if i + 1 < mentions.len() {
                    mentions[i + 1].start()
                } else {
                    input.len()
                };
                let prompt = input[start..end].trim().to_string();

                if !prompt.is_empty() {
                    tasks.push(ParsedAgentTask {
                        agent_id,
                        prompt,
                        explicit_mention: true,
                    });
                }
            }
        }

        Ok(tasks)
    }

    /// Route semantically based on keywords
    fn route_semantically(&self, input: &str) -> Option<String> {
        let input_lower = input.to_lowercase();

        // Find agent with highest priority that matches
        let mut best_match: Option<(&str, u32)> = None;

        for agent in self.agents.values() {
            if !agent.enabled {
                continue;
            }

            // Check keywords
            for keyword in &agent.routing.keywords {
                if input_lower.contains(&keyword.to_lowercase()) {
                    let priority = agent.routing.priority;
                    if best_match.is_none() || priority > best_match.map(|(_, p)| p).unwrap_or(0) {
                        best_match = Some((&agent.id, priority));
                    }
                    break;
                }
            }
        }

        best_match.map(|(id, _)| id.to_string())
    }

    /// Execute a single task
    async fn execute_task(
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

        let start = std::time::Instant::now();

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
    async fn execute_parallel(
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
                    // Skip cancelled tasks
                    continue;
                }
                Err(e) => {
                    warn!(error = %e, "Parallel task failed");
                    // Continue with other tasks
                }
            }
        }

        Ok(responses)
    }

    /// Get task status
    pub async fn get_task_status(&self, agent_id: &str) -> Option<TaskStatus> {
        let tasks = self.active_tasks.read().await;
        tasks.get(agent_id).cloned()
    }

    /// Clear completed tasks
    pub async fn clear_completed(&self) {
        let mut tasks = self.active_tasks.write().await;
        tasks.retain(|_, status| matches!(status, TaskStatus::Running | TaskStatus::Pending));
    }
}

impl Default for AgentOrchestrator {
    fn default() -> Self {
        Self::new(OrchestratorConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_mention() {
        let orchestrator = AgentOrchestrator::default();
        let tasks = orchestrator
            .parse_input("@backend implement the API")
            .unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].agent_id, "backend");
        assert_eq!(tasks[0].prompt, "implement the API");
        assert!(tasks[0].explicit_mention);
    }

    #[test]
    fn test_parse_multiple_mentions() {
        let orchestrator = AgentOrchestrator::default();
        let tasks = orchestrator
            .parse_input("@backend implement API @frontend create UI")
            .unwrap();

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].agent_id, "backend");
        assert_eq!(tasks[1].agent_id, "frontend");
    }

    #[test]
    fn test_parse_no_mention_semantic() {
        let orchestrator = AgentOrchestrator::default();
        let tasks = orchestrator.parse_input("design the API").unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].agent_id, "backend"); // Matches "API" keyword
        assert!(!tasks[0].explicit_mention);
    }

    #[test]
    fn test_parse_no_mention_default() {
        let orchestrator = AgentOrchestrator::default();
        let tasks = orchestrator.parse_input("do something").unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].agent_id, "researcher"); // Default agent
        assert!(!tasks[0].explicit_mention);
    }

    #[test]
    fn test_unknown_agent() {
        let orchestrator = AgentOrchestrator::default();
        let result = orchestrator.parse_input("@unknown_agent do something");

        assert!(result.is_err());
        if let Err(OrchestratorError::AgentNotFound(id)) = result {
            assert_eq!(id, "unknown_agent");
        }
    }

    #[test]
    fn test_list_agents() {
        let orchestrator = AgentOrchestrator::default();
        let agents = orchestrator.list_agents();

        assert!(agents.contains(&"backend"));
        assert!(agents.contains(&"frontend"));
        assert!(agents.contains(&"qa"));
        assert!(agents.contains(&"pm"));
        assert!(agents.contains(&"researcher"));
    }

    #[test]
    fn test_cancellation() {
        let orchestrator = AgentOrchestrator::default();

        assert!(!orchestrator.is_cancelled());

        orchestrator.cancel();

        assert!(orchestrator.is_cancelled());
    }

    #[test]
    fn test_token_tracking() {
        let config = OrchestratorConfig {
            token_budget: 1000,
            ..Default::default()
        };
        let orchestrator = AgentOrchestrator::new(config);

        assert_eq!(orchestrator.tokens_used(), 0);
        assert_eq!(orchestrator.tokens_remaining(), 1000);

        // Track some tokens
        orchestrator.track_tokens(500).unwrap();
        assert_eq!(orchestrator.tokens_used(), 500);
        assert_eq!(orchestrator.tokens_remaining(), 500);

        // Try to exceed budget
        let result = orchestrator.track_tokens(600);
        assert!(result.is_err());

        if let Err(OrchestratorError::BudgetExceeded { used, budget }) = result {
            assert_eq!(used, 1100);
            assert_eq!(budget, 1000);
        }
    }

    #[test]
    fn test_unlimited_budget() {
        let config = OrchestratorConfig {
            token_budget: 0, // Unlimited
            ..Default::default()
        };
        let orchestrator = AgentOrchestrator::new(config);

        assert_eq!(orchestrator.tokens_remaining(), u64::MAX);

        // Should allow any amount of tokens
        orchestrator.track_tokens(1_000_000).unwrap();
        orchestrator.track_tokens(1_000_000).unwrap();
    }

    #[test]
    fn test_recursion_depth() {
        let config = OrchestratorConfig {
            max_depth: 2,
            ..Default::default()
        };
        let orchestrator = AgentOrchestrator::new(config);

        // First two levels should work
        orchestrator.enter_depth().unwrap();
        orchestrator.enter_depth().unwrap();

        // Third level should fail
        let result = orchestrator.enter_depth();
        assert!(result.is_err());
        if let Err(OrchestratorError::MaxDepthExceeded(depth)) = result {
            assert_eq!(depth, 2);
        }

        // Exit depth
        orchestrator.exit_depth();
        orchestrator.exit_depth();

        // Should work again
        orchestrator.enter_depth().unwrap();
    }

    #[test]
    fn test_reset() {
        let orchestrator = AgentOrchestrator::default();

        orchestrator.track_tokens(5000).ok();
        orchestrator.enter_depth().ok();

        assert!(orchestrator.tokens_used() > 0);

        orchestrator.reset();

        assert_eq!(orchestrator.tokens_used(), 0);
    }
}
