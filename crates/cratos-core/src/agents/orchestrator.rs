//! Agent Orchestrator
//!
//! Unified multi-agent orchestration system for Cratos.
//!
//! ## Usage
//!
//! ```text
//! @backend API 구현해줘
//! @frontend UI 만들어줘
//! @backend API @frontend UI 병렬 실행
//! ```

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

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

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            default_agent: default_agent(),
            semantic_routing: true,
            workspace_base: default_workspace(),
            max_parallel: default_max_parallel(),
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
pub struct AgentOrchestrator {
    /// Registered agents
    agents: HashMap<String, AgentConfig>,
    /// CLI provider registry
    cli_registry: Arc<CliRegistry>,
    /// Configuration
    config: OrchestratorConfig,
    /// Active tasks
    active_tasks: RwLock<HashMap<String, TaskStatus>>,
    /// Agent mention regex
    mention_regex: Regex,
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
            mention_regex: Regex::new(r"@(\w+)\s+").expect("Invalid regex"),
        }
    }

    /// Create with custom CLI registry
    pub fn with_cli_registry(config: OrchestratorConfig, cli_registry: CliRegistry) -> Self {
        let mut orchestrator = Self::new(config);
        orchestrator.cli_registry = Arc::new(cli_registry);
        orchestrator
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
    /// let response = orchestrator.handle("@backend API 구현해줘", &context).await?;
    /// ```
    pub async fn handle(
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
    /// - Explicit @mentions: "@backend API 구현"
    /// - Multiple mentions: "@backend API @frontend UI"
    /// - Semantic routing (if no mention): "API 구현" → backend agent
    pub fn parse_input(&self, input: &str) -> OrchestratorResult<Vec<ParsedAgentTask>> {
        let mut tasks = Vec::new();

        // Find all @mentions
        let mentions: Vec<_> = self.mention_regex.find_iter(input).collect();

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
                    if best_match.is_none() || priority > best_match.unwrap().1 {
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
            OrchestratorError::Configuration(format!("CLI provider '{}' not found", agent.cli.provider))
        })?;

        // Determine workspace
        let workspace = context.workspace.clone().unwrap_or_else(|| {
            self.config
                .workspace_base
                .join(&context.session_id)
                .join(&task.agent_id)
        });

        // Execute via CLI provider
        let result = provider
            .execute(&task.prompt, &agent.persona.prompt, Some(&workspace))
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        let response = match result {
            Ok(content) => {
                info!(
                    agent_id = %task.agent_id,
                    duration_ms = duration_ms,
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
                tasks.insert(task.agent_id.clone(), TaskStatus::Completed(response.clone()));
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

        // Execute all (limited concurrency could be added here)
        let results = futures::future::join_all(futures).await;

        // Collect results
        let mut responses = Vec::new();
        for result in results {
            match result {
                Ok(response) => responses.push(response),
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
        let tasks = orchestrator.parse_input("@backend API 구현해줘").unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].agent_id, "backend");
        assert_eq!(tasks[0].prompt, "API 구현해줘");
        assert!(tasks[0].explicit_mention);
    }

    #[test]
    fn test_parse_multiple_mentions() {
        let orchestrator = AgentOrchestrator::default();
        let tasks = orchestrator
            .parse_input("@backend API 구현 @frontend UI 만들어줘")
            .unwrap();

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].agent_id, "backend");
        assert_eq!(tasks[1].agent_id, "frontend");
    }

    #[test]
    fn test_parse_no_mention_semantic() {
        let orchestrator = AgentOrchestrator::default();
        let tasks = orchestrator.parse_input("API 설계해줘").unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].agent_id, "backend"); // Matches "API" keyword
        assert!(!tasks[0].explicit_mention);
    }

    #[test]
    fn test_parse_no_mention_default() {
        let orchestrator = AgentOrchestrator::default();
        let tasks = orchestrator.parse_input("뭔가 해줘").unwrap();

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
}
