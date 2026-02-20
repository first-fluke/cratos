//! Agent Orchestrator
//!
//! Unified multi-agent orchestration system for Cratos.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::info;

mod budget;
mod execution;
mod parsing;
mod types;

pub use types::{
    AgentResponse, ExecutionContext, OrchestratorConfig, OrchestratorError, OrchestratorResult,
    ParsedAgentTask, TaskStatus,
};

use super::cli_registry::CliRegistry;
use super::config::AgentConfig;

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
mod tests;
