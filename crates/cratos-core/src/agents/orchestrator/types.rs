use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::LazyLock;
use thiserror::Error;

use super::super::cli_registry::CliError;

/// Pre-compiled regex for @mention parsing (e.g., "@backend do something")
pub(crate) static MENTION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@(\w+)\s+").expect("MENTION_REGEX is a compile-time constant"));

/// Default token budget per session (100K tokens)
pub(crate) const DEFAULT_TOKEN_BUDGET: u64 = 100_000;

/// Default max recursion depth for agent calls
pub(crate) const DEFAULT_MAX_DEPTH: u32 = 3;

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
