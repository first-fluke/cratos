//! Orchestrator configuration
//!
//! Contains configuration types for the orchestrator:
//! - `OrchestratorConfig` for orchestrator settings
//! - `OrchestratorInput` for execution input

use crate::memory::SessionContext;
use crate::planner::PlannerConfig;
use cratos_tools::RunnerConfig;

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
    /// Override the system prompt entirely (e.g., for /develop workflow)
    pub system_prompt_override: Option<String>,
    /// Inline images from the channel (e.g., Telegram photo messages)
    pub images: Vec<cratos_llm::ImageContent>,
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
            system_prompt_override: None,
            images: Vec::new(),
        }
    }

    /// Set the thread ID
    #[must_use]
    pub fn with_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Add inline images
    #[must_use]
    pub fn with_images(mut self, images: Vec<cratos_llm::ImageContent>) -> Self {
        self.images = images;
        self
    }

    /// Override the system prompt (e.g., for workflow-driven execution)
    #[must_use]
    pub fn with_system_prompt_override(mut self, prompt: String) -> Self {
        self.system_prompt_override = Some(prompt);
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
    /// M4: Bail out after this many total failures across all iterations
    pub max_total_failures: usize,
    /// Enable automatic skill pattern detection
    pub auto_skill_detection: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            enable_logging: true,
            planner_config: PlannerConfig::default(),
            runner_config: RunnerConfig::default(),
            max_execution_secs: 180,
            max_consecutive_failures: 3,
            max_total_failures: 6,
            auto_skill_detection: true,
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

    /// Set auto skill detection (M6: Auto Skill Generation)
    #[must_use]
    pub fn with_auto_skill_detection(mut self, enabled: bool) -> Self {
        self.auto_skill_detection = enabled;
        self
    }
}
