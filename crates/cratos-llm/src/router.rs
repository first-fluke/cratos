//! Router - LLM Provider abstraction and routing
//!
//! This module defines the core traits and types for LLM providers,
//! as well as the router for selecting providers based on configuration.
//!
//! ## Token Management
//!
//! The router includes intelligent token management features:
//! - **Task-specific token budgets**: Different tasks get appropriate max_tokens
//! - **Token counting**: Accurate client-side token estimation using tiktoken
//! - **Cost estimation**: Relative cost calculation based on model tier

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument};

// Re-export types from submodules for backward compatibility
pub use crate::completion::{
    CompletionRequest, CompletionResponse, TokenUsage, ToolCompletionRequest,
    ToolCompletionResponse,
};
pub use crate::message::{Message, MessageRole};
pub use crate::token::{
    count_message_tokens, count_tokens, TokenBudget, TokenCounter, TOKEN_COUNTER,
};
pub use crate::tools::{ToolCall, ToolChoice, ToolDefinition};

// ============================================================================
// Task Type and Model Routing
// ============================================================================

/// Task type for intelligent model routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Simple classification or intent detection
    Classification,
    /// Complex planning and reasoning
    Planning,
    /// Code generation and modification
    CodeGeneration,
    /// Text summarization
    Summarization,
    /// General conversation
    Conversation,
    /// Data extraction and parsing
    Extraction,
    /// Code review and analysis
    CodeReview,
    /// Translation
    Translation,
}

impl TaskType {
    /// Get the recommended model tier for this task type
    ///
    /// Tier selection optimizes cost while maintaining quality:
    /// - UltraBudget: Trivial tasks (classification, extraction, translation)
    /// - Fast: Simple tasks (summarization)
    /// - Standard: General tasks (conversation)
    /// - Premium: Complex tasks (planning, code generation, code review)
    #[must_use]
    pub fn recommended_tier(&self) -> ModelTier {
        match self {
            // UltraBudget: Trivial tasks that don't need advanced reasoning
            Self::Classification => ModelTier::UltraBudget,
            Self::Extraction => ModelTier::UltraBudget,
            Self::Translation => ModelTier::UltraBudget,

            // Fast: Simple tasks requiring basic comprehension
            Self::Summarization => ModelTier::Fast,

            // Standard: General tasks requiring good language understanding
            Self::Conversation => ModelTier::Standard,

            // Premium: Complex tasks requiring advanced reasoning
            Self::Planning => ModelTier::Premium,
            Self::CodeGeneration => ModelTier::Premium,
            Self::CodeReview => ModelTier::Premium,
        }
    }

    /// Whether this task type requires tool support
    #[must_use]
    pub fn requires_tools(&self) -> bool {
        matches!(
            self,
            Self::Planning | Self::CodeGeneration | Self::CodeReview
        )
    }

    /// Get the default token budget for this task type
    ///
    /// Token budgets are optimized based on typical response lengths:
    /// - Classification: Short labels/categories (200 tokens)
    /// - Extraction: Structured data extraction (500 tokens)
    /// - Summarization: Condensed summaries (1000 tokens)
    /// - Translation: Similar length to input (800 tokens)
    /// - Conversation: General chat responses (2000 tokens)
    /// - Planning: Detailed step-by-step plans (3000 tokens)
    /// - CodeReview: Analysis with suggestions (3000 tokens)
    /// - CodeGeneration: Full code implementations (4096 tokens)
    #[must_use]
    pub fn default_token_budget(&self) -> TokenBudget {
        match self {
            Self::Classification => TokenBudget::new(200, 0.3),
            Self::Extraction => TokenBudget::new(500, 0.2),
            Self::Summarization => TokenBudget::new(1000, 0.5),
            Self::Translation => TokenBudget::new(800, 0.3),
            Self::Conversation => TokenBudget::new(2000, 0.7),
            Self::Planning => TokenBudget::new(3000, 0.7),
            Self::CodeReview => TokenBudget::new(3000, 0.5),
            Self::CodeGeneration => TokenBudget::new(4096, 0.7),
        }
    }
}

/// Model tier for cost/performance optimization
///
/// Tiers are ordered by cost (ascending) and quality (ascending):
/// - UltraBudget: < $0.15/M tokens (DeepSeek R1 Distill, Qwen 2.5)
/// - Fast: $0.15 ~ $1.00/M tokens (GPT-5 nano, Gemini 3 Flash)
/// - Standard: $1.00 ~ $5.00/M tokens (Claude Sonnet 4, Gemini 3 Pro)
/// - Premium: > $5.00/M tokens (Claude Opus 4.5, GPT-5 Ultra)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    /// Ultra-budget models for trivial tasks (< $0.15/M tokens)
    /// Examples: DeepSeek R1 Distill ($0.03), Qwen 2.5-VL ($0.05), GLM-4-9B ($0.086)
    UltraBudget,
    /// Fast, cheap models for simple tasks ($0.15 ~ $1.00/M tokens)
    /// Examples: GPT-5 nano, Gemini 3 Flash, Mistral Medium 3
    Fast,
    /// Balanced models for general tasks ($1.00 ~ $5.00/M tokens)
    /// Examples: Claude Sonnet 4, Gemini 3 Pro, GPT-5.2
    Standard,
    /// Premium models for complex reasoning (> $5.00/M tokens)
    /// Examples: Claude Opus 4.5, GPT-5 Ultra
    Premium,
}

impl ModelTier {
    /// Get default model for each provider at this tier
    ///
    /// Model selection follows 2026 pricing tiers:
    /// - UltraBudget: DeepSeek R1 Distill, Qwen 2.5, GLM-4-9B
    /// - Fast: Gemini 3 Flash, GPT-5 nano, Llama 4 Scout
    /// - Standard: Claude Sonnet 4, Gemini 3 Pro, GPT-5.2
    /// - Premium: Claude Opus 4.5, GPT-5 Ultra
    #[must_use]
    pub fn default_model(&self, provider: &str) -> &'static str {
        match (self, provider) {
            // ================================================================
            // DeepSeek - Ultra-low-cost leader ($0.03 ~ $0.55/M tokens)
            // ================================================================
            (ModelTier::UltraBudget, "deepseek") => "deepseek-r1-distill-llama-70b",
            (ModelTier::Fast, "deepseek") => "deepseek-chat",
            (ModelTier::Standard, "deepseek") => "deepseek-chat",
            (ModelTier::Premium, "deepseek") => "deepseek-reasoner",

            // ================================================================
            // Groq - FREE tier with Llama 4 (rate limited)
            // ================================================================
            (ModelTier::UltraBudget, "groq") => "llama-4-scout-17b-16e-instruct",
            (ModelTier::Fast, "groq") => "llama-4-scout-17b-16e-instruct",
            (ModelTier::Standard, "groq") => "llama-4-maverick-17b-128e-instruct",
            (ModelTier::Premium, "groq") => "llama-4-maverick-17b-128e-instruct",

            // ================================================================
            // OpenAI - GPT-5 family (2026)
            // ================================================================
            (ModelTier::UltraBudget, "openai") => "gpt-5-nano",
            (ModelTier::Fast, "openai") => "gpt-5-nano",
            (ModelTier::Standard, "openai") => "gpt-5.2",
            (ModelTier::Premium, "openai") => "gpt-5-ultra",

            // ================================================================
            // Anthropic - Claude 4 family (2026)
            // ================================================================
            (ModelTier::UltraBudget, "anthropic") => "claude-3-5-haiku-20241022",
            (ModelTier::Fast, "anthropic") => "claude-3-5-haiku-20241022",
            (ModelTier::Standard, "anthropic") => "claude-sonnet-4-20250514",
            (ModelTier::Premium, "anthropic") => "claude-opus-4-20250514",

            // ================================================================
            // Google Gemini - Gemini 3 family (2026)
            // ================================================================
            (ModelTier::UltraBudget, "gemini") => "gemini-3-flash",
            (ModelTier::Fast, "gemini") => "gemini-3-flash",
            (ModelTier::Standard, "gemini") => "gemini-3-pro",
            (ModelTier::Premium, "gemini") => "gemini-3-pro",

            // ================================================================
            // GLM (ZhipuAI) - Chinese models
            // ================================================================
            (ModelTier::UltraBudget, "glm") => "glm-4-9b",
            (ModelTier::Fast, "glm") => "glm-4-flash",
            (ModelTier::Standard, "glm") => "glm-4-plus",
            (ModelTier::Premium, "glm") => "glm-4-plus",

            // ================================================================
            // Qwen (Alibaba) - Qwen 3 family (2026)
            // ================================================================
            (ModelTier::UltraBudget, "qwen") => "qwen3-8b",
            (ModelTier::Fast, "qwen") => "qwen-turbo",
            (ModelTier::Standard, "qwen") => "qwen3-32b",
            (ModelTier::Premium, "qwen") => "qwen-max",

            // ================================================================
            // OpenRouter - Multi-provider gateway
            // ================================================================
            (ModelTier::UltraBudget, "openrouter") => "deepseek/deepseek-r1-distill-llama-70b",
            (ModelTier::Fast, "openrouter") => "qwen/qwen3-32b:free",
            (ModelTier::Standard, "openrouter") => "google/gemini-3-flash",
            (ModelTier::Premium, "openrouter") => "anthropic/claude-sonnet-4",

            // ================================================================
            // Novita - Free tier provider
            // ================================================================
            (ModelTier::UltraBudget, "novita") => "qwen/qwen2.5-7b-instruct",
            (ModelTier::Fast, "novita") => "qwen/qwen2.5-7b-instruct",
            (ModelTier::Standard, "novita") => "thudm/glm-4-9b-chat",
            (ModelTier::Premium, "novita") => "qwen/qwen2.5-72b-instruct",

            // ================================================================
            // SiliconFlow - Cheapest provider ($0.03 ~ $0.09/M tokens)
            // ================================================================
            (ModelTier::UltraBudget, "siliconflow") => "deepseek-ai/DeepSeek-R1-Distill-Llama-70B",
            (ModelTier::Fast, "siliconflow") => "Qwen/Qwen2.5-VL-7B-Instruct",
            (ModelTier::Standard, "siliconflow") => "meta-llama/Llama-3.1-70B-Instruct",
            (ModelTier::Premium, "siliconflow") => "meta-llama/Llama-3.1-70B-Instruct",

            // ================================================================
            // Fireworks - Fast inference
            // ================================================================
            (ModelTier::UltraBudget, "fireworks") => {
                "accounts/fireworks/models/llama-v3p1-8b-instruct"
            }
            (ModelTier::Fast, "fireworks") => "accounts/fireworks/models/llama-v3p1-70b-instruct",
            (ModelTier::Standard, "fireworks") => {
                "accounts/fireworks/models/llama-v3p1-405b-instruct"
            }
            (ModelTier::Premium, "fireworks") => {
                "accounts/fireworks/models/llama-v3p1-405b-instruct"
            }

            // ================================================================
            // Ollama - Local models (all same model)
            // ================================================================
            (_, "ollama") => "llama3.2",

            // ================================================================
            // Default fallback
            // ================================================================
            _ => "gpt-5.2",
        }
    }

    /// Estimated cost multiplier relative to Fast tier
    ///
    /// Based on 2026 pricing:
    /// - UltraBudget: ~$0.05/M → 0.2x
    /// - Fast: ~$0.50/M → 1.0x (baseline)
    /// - Standard: ~$3.00/M → 6.0x
    /// - Premium: ~$15.00/M → 30.0x
    #[must_use]
    pub fn cost_multiplier(&self) -> f32 {
        match self {
            ModelTier::UltraBudget => 0.1,
            ModelTier::Fast => 1.0,
            ModelTier::Standard => 6.0,
            ModelTier::Premium => 30.0,
        }
    }

    /// Get the price range description for this tier
    #[must_use]
    pub fn price_range(&self) -> &'static str {
        match self {
            ModelTier::UltraBudget => "< $0.15/M tokens",
            ModelTier::Fast => "$0.15 ~ $1.00/M tokens",
            ModelTier::Standard => "$1.00 ~ $5.00/M tokens",
            ModelTier::Premium => "> $5.00/M tokens",
        }
    }

    /// Constrain this tier to not exceed the given maximum tier
    ///
    /// Tier ordering: UltraBudget < Fast < Standard < Premium
    #[must_use]
    pub fn constrain_to(&self, max_tier: &ModelTier) -> ModelTier {
        let self_level = self.level();
        let max_level = max_tier.level();

        if self_level <= max_level {
            *self
        } else {
            *max_tier
        }
    }

    /// Get numeric level for tier comparison (lower = cheaper)
    #[must_use]
    fn level(&self) -> u8 {
        match self {
            ModelTier::UltraBudget => 0,
            ModelTier::Fast => 1,
            ModelTier::Standard => 2,
            ModelTier::Premium => 3,
        }
    }
}

/// Model routing configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingRules {
    /// Task-specific provider overrides
    #[serde(default)]
    pub task_providers: HashMap<TaskType, String>,
    /// Task-specific model overrides
    #[serde(default)]
    pub task_models: HashMap<TaskType, String>,
    /// Task-specific token budget overrides
    #[serde(default)]
    pub task_token_budgets: HashMap<TaskType, TokenBudget>,
    /// Whether to prefer local models when available
    #[serde(default)]
    pub prefer_local: bool,
    /// Maximum cost tier allowed
    #[serde(default)]
    pub max_tier: Option<ModelTier>,
}

impl RoutingRules {
    /// Get token budget for a task type, with custom override or default
    #[must_use]
    pub fn get_token_budget(&self, task_type: TaskType) -> TokenBudget {
        self.task_token_budgets
            .get(&task_type)
            .cloned()
            .unwrap_or_else(|| task_type.default_token_budget())
    }
}

// ============================================================================
// Provider Trait
// ============================================================================

/// Trait for LLM providers
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;

    /// Check if the provider supports function calling/tools
    fn supports_tools(&self) -> bool;

    /// Get available models
    fn available_models(&self) -> Vec<String>;

    /// Get the default model
    fn default_model(&self) -> &str;

    /// Complete a conversation (text only)
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Complete a conversation with tools
    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse>;
}

// ============================================================================
// LLM Router
// ============================================================================

/// Configuration for a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Whether the provider is enabled
    pub enabled: bool,
    /// API key (or env var name)
    pub api_key: Option<String>,
    /// Base URL override
    pub base_url: Option<String>,
    /// Default model
    pub default_model: Option<String>,
    /// Request timeout in milliseconds
    pub timeout_ms: Option<u64>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key: None,
            base_url: None,
            default_model: None,
            timeout_ms: Some(60_000),
        }
    }
}

/// Router configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Default provider name
    pub default_provider: String,
    /// Provider-specific configurations
    pub providers: HashMap<String, ProviderConfig>,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            default_provider: "openai".to_string(),
            providers: HashMap::new(),
        }
    }
}

/// LLM Router for managing multiple providers with intelligent routing
pub struct LlmRouter {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
    default_provider: String,
    routing_rules: RoutingRules,
}

impl LlmRouter {
    /// Create a new router
    #[must_use]
    pub fn new(default_provider: impl Into<String>) -> Self {
        Self {
            providers: HashMap::new(),
            default_provider: default_provider.into(),
            routing_rules: RoutingRules::default(),
        }
    }

    /// Create a router with routing rules
    #[must_use]
    pub fn with_routing_rules(mut self, rules: RoutingRules) -> Self {
        self.routing_rules = rules;
        self
    }

    /// Set routing rules
    pub fn set_routing_rules(&mut self, rules: RoutingRules) {
        self.routing_rules = rules;
    }

    /// Get the routing rules
    #[must_use]
    pub fn routing_rules(&self) -> &RoutingRules {
        &self.routing_rules
    }

    /// Register a provider
    pub fn register(&mut self, name: impl Into<String>, provider: Arc<dyn LlmProvider>) {
        let name = name.into();
        debug!(provider = %name, "Registering LLM provider");
        self.providers.insert(name, provider);
    }

    /// Get a provider by name
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn LlmProvider>> {
        self.providers.get(name).cloned()
    }

    /// Get the default provider
    #[must_use]
    pub fn default_provider(&self) -> Option<Arc<dyn LlmProvider>> {
        self.get(&self.default_provider)
    }

    /// Get the default provider name
    #[must_use]
    pub fn default_provider_name(&self) -> &str {
        &self.default_provider
    }

    /// Set the default provider
    pub fn set_default(&mut self, name: impl Into<String>) {
        self.default_provider = name.into();
    }

    /// List registered provider names
    #[must_use]
    pub fn list_providers(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a provider is registered
    #[must_use]
    pub fn has_provider(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }

    /// Complete using the default provider
    #[instrument(skip(self, request))]
    pub async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let provider = self
            .default_provider()
            .ok_or_else(|| Error::NotConfigured(self.default_provider.clone()))?;

        provider.complete(request).await
    }

    /// Complete with tools using the default provider
    #[instrument(skip(self, request))]
    pub async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse> {
        let provider = self
            .default_provider()
            .ok_or_else(|| Error::NotConfigured(self.default_provider.clone()))?;

        if !provider.supports_tools() {
            return Err(Error::Api(format!(
                "Provider {} does not support tools",
                provider.name()
            )));
        }

        provider.complete_with_tools(request).await
    }

    /// Complete using a specific provider
    #[instrument(skip(self, request))]
    pub async fn complete_with(
        &self,
        provider_name: &str,
        request: CompletionRequest,
    ) -> Result<CompletionResponse> {
        let provider = self
            .get(provider_name)
            .ok_or_else(|| Error::NotConfigured(provider_name.to_string()))?;

        provider.complete(request).await
    }

    /// Complete with tools using a specific provider
    #[instrument(skip(self, request))]
    pub async fn complete_with_tools_using(
        &self,
        provider_name: &str,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse> {
        let provider = self
            .get(provider_name)
            .ok_or_else(|| Error::NotConfigured(provider_name.to_string()))?;

        if !provider.supports_tools() {
            return Err(Error::Api(format!(
                "Provider {} does not support tools",
                provider.name()
            )));
        }

        provider.complete_with_tools(request).await
    }

    // ========================================================================
    // Task-based Routing (Intelligent Model Selection)
    // ========================================================================

    /// Select the best provider and model for a task type
    #[must_use]
    pub fn select_for_task(&self, task_type: TaskType) -> Option<(Arc<dyn LlmProvider>, String)> {
        // Check for task-specific provider override
        if let Some(provider_name) = self.routing_rules.task_providers.get(&task_type) {
            if let Some(provider) = self.get(provider_name) {
                let model = self
                    .routing_rules
                    .task_models
                    .get(&task_type)
                    .cloned()
                    .unwrap_or_else(|| provider.default_model().to_string());
                return Some((provider, model));
            }
        }

        // Check if local models are preferred and available
        if self.routing_rules.prefer_local {
            if let Some(provider) = self.get("ollama") {
                let model = self
                    .routing_rules
                    .task_models
                    .get(&task_type)
                    .cloned()
                    .unwrap_or_else(|| provider.default_model().to_string());
                return Some((provider, model));
            }
        }

        // Get recommended tier for task
        let mut tier = task_type.recommended_tier();

        // Apply max tier constraint (downgrade if necessary)
        if let Some(max_tier) = &self.routing_rules.max_tier {
            tier = tier.constrain_to(max_tier);
        }

        // Check if task requires tools
        let requires_tools = task_type.requires_tools();

        // Find best provider
        let provider = self.default_provider()?;
        let provider_name = self.default_provider_name();

        // Skip providers that don't support tools if needed
        if requires_tools && !provider.supports_tools() {
            // Try to find a provider that supports tools
            for name in self.list_providers() {
                if let Some(p) = self.get(name) {
                    if p.supports_tools() {
                        let model = tier.default_model(name).to_string();
                        info!(
                            task = ?task_type,
                            provider = name,
                            model = %model,
                            tier = ?tier,
                            "Selected model for task (tool support required)"
                        );
                        return Some((p, model));
                    }
                }
            }
            return None;
        }

        // Get model for tier
        let model = self
            .routing_rules
            .task_models
            .get(&task_type)
            .cloned()
            .unwrap_or_else(|| tier.default_model(provider_name).to_string());

        info!(
            task = ?task_type,
            provider = provider_name,
            model = %model,
            tier = ?tier,
            "Selected model for task"
        );

        Some((provider, model))
    }

    /// Complete a request with automatic model selection based on task type
    ///
    /// This method automatically applies task-specific token budgets:
    /// - Classification: 200 tokens (short labels)
    /// - Extraction: 500 tokens (structured data)
    /// - Summarization: 1000 tokens (condensed text)
    /// - Translation: 800 tokens (similar to input)
    /// - Conversation: 2000 tokens (general chat)
    /// - Planning: 3000 tokens (detailed plans)
    /// - CodeReview: 3000 tokens (analysis)
    /// - CodeGeneration: 4096 tokens (full implementations)
    #[instrument(skip(self, messages))]
    pub async fn complete_for_task(
        &self,
        task_type: TaskType,
        messages: Vec<Message>,
    ) -> Result<CompletionResponse> {
        let (provider, model) = self
            .select_for_task(task_type)
            .ok_or_else(|| Error::NotConfigured("No suitable provider found".to_string()))?;

        // Get task-specific token budget (with custom override support)
        let budget = self.routing_rules.get_token_budget(task_type);

        info!(
            task = ?task_type,
            max_tokens = budget.max_tokens,
            temperature = budget.temperature,
            "Applying task-specific token budget"
        );

        let request = CompletionRequest {
            model,
            messages,
            max_tokens: Some(budget.max_tokens),
            temperature: Some(budget.temperature),
            stop: None,
        };

        provider.complete(request).await
    }

    /// Complete with tools using automatic model selection based on task type
    ///
    /// Applies task-specific token budgets automatically.
    /// See `complete_for_task` for budget details.
    #[instrument(skip(self, messages, tools))]
    pub async fn complete_with_tools_for_task(
        &self,
        task_type: TaskType,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> Result<ToolCompletionResponse> {
        let (provider, model) = self
            .select_for_task(task_type)
            .ok_or_else(|| Error::NotConfigured("No suitable provider found".to_string()))?;

        if !provider.supports_tools() {
            return Err(Error::Api(format!(
                "Provider {} does not support tools",
                provider.name()
            )));
        }

        // Get task-specific token budget (with custom override support)
        let budget = self.routing_rules.get_token_budget(task_type);

        info!(
            task = ?task_type,
            max_tokens = budget.max_tokens,
            temperature = budget.temperature,
            "Applying task-specific token budget for tool completion"
        );

        let request = ToolCompletionRequest {
            request: CompletionRequest {
                model,
                messages,
                max_tokens: Some(budget.max_tokens),
                temperature: Some(budget.temperature),
                stop: None,
            },
            tools,
            tool_choice: ToolChoice::Auto,
        };

        provider.complete_with_tools(request).await
    }

    /// Estimate cost for a task (relative units)
    #[must_use]
    pub fn estimate_cost(&self, task_type: TaskType, estimated_tokens: u32) -> f32 {
        let tier = task_type.recommended_tier();
        let multiplier = tier.cost_multiplier();
        (estimated_tokens as f32 / 1000.0) * multiplier
    }
}

// ============================================================================
// Model Routing Configuration (Cost Optimization)
// ============================================================================

/// Model configuration for a specific tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Provider name (e.g., "groq", "deepseek", "anthropic")
    pub provider: String,
    /// Model name (e.g., "llama-3.3-70b-versatile", "deepseek-chat")
    pub model: String,
}

impl ModelConfig {
    /// Create a new model configuration
    #[must_use]
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
        }
    }
}

/// Model routing configuration for cost optimization
///
/// Cratos uses a 4-tiered approach to minimize costs (2026 pricing):
/// - Trivial tasks: UltraBudget (DeepSeek R1 Distill) - $0.03/1M tokens
/// - Simple tasks: Fast (Gemini 3 Flash) - $0.50/1M tokens
/// - General tasks: Standard (Claude Sonnet 4) - $3.00/1M tokens
/// - Complex tasks: Premium (Claude Opus 4.5) - $15.00/1M tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoutingConfig {
    /// Model for trivial tasks (classification, extraction, translation)
    /// Default: DeepSeek R1 Distill ($0.03/1M)
    pub trivial: ModelConfig,
    /// Model for simple tasks (summarization)
    /// Default: Gemini 3 Flash ($0.50/1M)
    pub simple: ModelConfig,
    /// Model for general tasks (conversation)
    /// Default: Claude Sonnet 4 ($3/1M)
    pub general: ModelConfig,
    /// Model for complex tasks (planning, code generation)
    /// Default: Claude Opus 4.5 ($15/1M)
    pub complex: ModelConfig,
    /// Fallback model when primary provider fails
    pub fallback: Option<ModelConfig>,
    /// Whether to automatically downgrade on rate limits
    #[serde(default = "default_true")]
    pub auto_downgrade: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ModelRoutingConfig {
    fn default() -> Self {
        Self {
            // UltraBudget - DeepSeek R1 Distill ($0.03/1M input, $0.09/1M output)
            trivial: ModelConfig::new("deepseek", "deepseek-r1-distill-llama-70b"),
            // Fast - Gemini 3 Flash ($0.50/1M, fastest inference)
            simple: ModelConfig::new("gemini", "gemini-3-flash"),
            // Standard - Claude Sonnet 4 ($3/1M, balanced quality)
            general: ModelConfig::new("anthropic", "claude-sonnet-4-20250514"),
            // Premium - Claude Opus 4.5 ($15/1M, highest quality)
            complex: ModelConfig::new("anthropic", "claude-opus-4-20250514"),
            // Fallback to GPT-5 nano if others fail
            fallback: Some(ModelConfig::new("openai", "gpt-5-nano")),
            auto_downgrade: true,
        }
    }
}

impl ModelRoutingConfig {
    /// Create a config optimized for free/low-cost usage
    ///
    /// Uses FREE providers (Groq, Novita) where possible:
    /// - Trivial: Groq Llama 4 Scout (FREE)
    /// - Simple: Groq Llama 4 Scout (FREE)
    /// - General: DeepSeek Chat ($0.14/1M)
    /// - Complex: DeepSeek Reasoner ($0.55/1M)
    #[must_use]
    pub fn free_tier() -> Self {
        Self {
            trivial: ModelConfig::new("groq", "llama-4-scout-17b-16e-instruct"),
            simple: ModelConfig::new("groq", "llama-4-scout-17b-16e-instruct"),
            general: ModelConfig::new("deepseek", "deepseek-chat"),
            complex: ModelConfig::new("deepseek", "deepseek-reasoner"),
            fallback: Some(ModelConfig::new("novita", "qwen/qwen2.5-72b-instruct")),
            auto_downgrade: true,
        }
    }

    /// Create a config optimized for quality (uses premium models)
    ///
    /// Uses Anthropic Claude 4 family for all tiers:
    /// - Trivial: Claude 3.5 Haiku ($0.25/1M)
    /// - Simple: Claude 3.5 Haiku ($0.25/1M)
    /// - General: Claude Sonnet 4 ($3/1M)
    /// - Complex: Claude Opus 4.5 ($15/1M)
    #[must_use]
    pub fn quality_tier() -> Self {
        Self {
            trivial: ModelConfig::new("anthropic", "claude-3-5-haiku-20241022"),
            simple: ModelConfig::new("anthropic", "claude-3-5-haiku-20241022"),
            general: ModelConfig::new("anthropic", "claude-sonnet-4-20250514"),
            complex: ModelConfig::new("anthropic", "claude-opus-4-20250514"),
            fallback: Some(ModelConfig::new("openai", "gpt-5.2")),
            auto_downgrade: false,
        }
    }

    /// Create a config for local-only usage (Ollama)
    ///
    /// All tiers use the same local model (Llama 3.2)
    #[must_use]
    pub fn local_only() -> Self {
        Self {
            trivial: ModelConfig::new("ollama", "llama3.2"),
            simple: ModelConfig::new("ollama", "llama3.2"),
            general: ModelConfig::new("ollama", "llama3.2"),
            complex: ModelConfig::new("ollama", "llama3.2"),
            fallback: None,
            auto_downgrade: false,
        }
    }

    /// Get the model config for a task type
    #[must_use]
    pub fn get_for_task(&self, task_type: TaskType) -> &ModelConfig {
        match task_type.recommended_tier() {
            ModelTier::UltraBudget => &self.trivial,
            ModelTier::Fast => &self.simple,
            ModelTier::Standard => &self.general,
            ModelTier::Premium => &self.complex,
        }
    }

    /// Estimate monthly cost for given usage pattern (in USD)
    ///
    /// # Arguments
    /// * `trivial_tokens` - Monthly tokens for trivial tasks (classification, extraction)
    /// * `simple_tokens` - Monthly tokens for simple tasks (summarization)
    /// * `general_tokens` - Monthly tokens for general tasks (conversation)
    /// * `complex_tokens` - Monthly tokens for complex tasks (planning, code)
    #[must_use]
    pub fn estimate_monthly_cost(
        &self,
        trivial_tokens: u64,
        simple_tokens: u64,
        general_tokens: u64,
        complex_tokens: u64,
    ) -> f64 {
        let trivial_price =
            Self::get_provider_price(&self.trivial.provider, ModelTier::UltraBudget);
        let simple_price = Self::get_provider_price(&self.simple.provider, ModelTier::Fast);
        let general_price = Self::get_provider_price(&self.general.provider, ModelTier::Standard);
        let complex_price = Self::get_provider_price(&self.complex.provider, ModelTier::Premium);

        let trivial_cost = (trivial_tokens as f64 / 1_000_000.0) * trivial_price;
        let simple_cost = (simple_tokens as f64 / 1_000_000.0) * simple_price;
        let general_cost = (general_tokens as f64 / 1_000_000.0) * general_price;
        let complex_cost = (complex_tokens as f64 / 1_000_000.0) * complex_price;

        trivial_cost + simple_cost + general_cost + complex_cost
    }

    /// Get average price per 1M tokens for a provider at a given tier (2026 pricing)
    fn get_provider_price(provider: &str, tier: ModelTier) -> f64 {
        match (provider, tier) {
            // Groq - FREE (rate limited)
            ("groq", _) => 0.0,
            // Novita - FREE tier
            ("novita", _) => 0.0,
            // DeepSeek - Ultra-low-cost
            ("deepseek", ModelTier::UltraBudget) => 0.06, // R1 Distill avg
            ("deepseek", ModelTier::Fast) => 0.21,        // Chat avg
            ("deepseek", ModelTier::Standard) => 0.21,    // Chat avg
            ("deepseek", ModelTier::Premium) => 1.37,     // Reasoner avg
            // SiliconFlow - Cheapest
            ("siliconflow", ModelTier::UltraBudget) => 0.06,
            ("siliconflow", _) => 0.10,
            // Fireworks
            ("fireworks", ModelTier::UltraBudget) => 0.10,
            ("fireworks", _) => 0.50,
            // OpenAI - GPT-5 family
            ("openai", ModelTier::UltraBudget) => 0.225, // GPT-5 nano avg
            ("openai", ModelTier::Fast) => 0.225,
            ("openai", ModelTier::Standard) => 6.25, // GPT-5.2 avg
            ("openai", ModelTier::Premium) => 20.0,  // GPT-5 Ultra avg
            // Anthropic - Claude 4 family
            ("anthropic", ModelTier::UltraBudget) => 0.40, // Haiku avg
            ("anthropic", ModelTier::Fast) => 0.40,
            ("anthropic", ModelTier::Standard) => 9.0, // Sonnet 4 avg
            ("anthropic", ModelTier::Premium) => 45.0, // Opus 4.5 avg
            // Gemini - Gemini 3 family
            ("gemini", ModelTier::UltraBudget) => 0.175, // Flash avg
            ("gemini", ModelTier::Fast) => 1.75,
            ("gemini", ModelTier::Standard) => 7.0, // Pro avg
            ("gemini", ModelTier::Premium) => 7.0,
            // GLM
            ("glm", ModelTier::UltraBudget) => 0.086,
            ("glm", _) => 0.20,
            // Qwen
            ("qwen", ModelTier::UltraBudget) => 0.075,
            ("qwen", _) => 0.50,
            // Ollama - FREE (local)
            ("ollama", _) => 0.0,
            // Default
            _ => 1.0,
        }
    }
}

// ============================================================================
// LlmProvider implementation for LlmRouter
// ============================================================================

#[async_trait::async_trait]
impl LlmProvider for LlmRouter {
    fn name(&self) -> &str {
        "router"
    }

    fn supports_tools(&self) -> bool {
        self.default_provider()
            .map(|p| p.supports_tools())
            .unwrap_or(false)
    }

    fn available_models(&self) -> Vec<String> {
        self.providers
            .values()
            .flat_map(|p| p.available_models())
            .collect()
    }

    fn default_model(&self) -> &str {
        &self.default_provider
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        LlmRouter::complete(self, request).await
    }

    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse> {
        LlmRouter::complete_with_tools(self, request).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router() {
        let router = LlmRouter::new("openai");
        assert_eq!(router.default_provider_name(), "openai");
        assert!(!router.has_provider("openai"));
        assert!(router.list_providers().is_empty());
    }

    #[test]
    fn test_task_type_recommended_tier() {
        // UltraBudget tier tasks (trivial)
        assert_eq!(
            TaskType::Classification.recommended_tier(),
            ModelTier::UltraBudget
        );
        assert_eq!(
            TaskType::Extraction.recommended_tier(),
            ModelTier::UltraBudget
        );
        assert_eq!(
            TaskType::Translation.recommended_tier(),
            ModelTier::UltraBudget
        );

        // Fast tier tasks (simple)
        assert_eq!(TaskType::Summarization.recommended_tier(), ModelTier::Fast);

        // Standard tier tasks (general)
        assert_eq!(
            TaskType::Conversation.recommended_tier(),
            ModelTier::Standard
        );

        // Premium tier tasks (complex)
        assert_eq!(TaskType::Planning.recommended_tier(), ModelTier::Premium);
        assert_eq!(
            TaskType::CodeGeneration.recommended_tier(),
            ModelTier::Premium
        );
        assert_eq!(TaskType::CodeReview.recommended_tier(), ModelTier::Premium);
    }

    #[test]
    fn test_task_type_requires_tools() {
        assert!(TaskType::Planning.requires_tools());
        assert!(TaskType::CodeGeneration.requires_tools());
        assert!(TaskType::CodeReview.requires_tools());

        assert!(!TaskType::Classification.requires_tools());
        assert!(!TaskType::Summarization.requires_tools());
        assert!(!TaskType::Conversation.requires_tools());
    }

    #[test]
    fn test_model_tier_default_model() {
        // OpenAI (GPT-5 family 2026)
        assert_eq!(ModelTier::UltraBudget.default_model("openai"), "gpt-5-nano");
        assert_eq!(ModelTier::Fast.default_model("openai"), "gpt-5-nano");
        assert_eq!(ModelTier::Standard.default_model("openai"), "gpt-5.2");
        assert_eq!(ModelTier::Premium.default_model("openai"), "gpt-5-ultra");

        // Anthropic (Claude 4 family 2026)
        assert_eq!(
            ModelTier::UltraBudget.default_model("anthropic"),
            "claude-3-5-haiku-20241022"
        );
        assert_eq!(
            ModelTier::Fast.default_model("anthropic"),
            "claude-3-5-haiku-20241022"
        );
        assert_eq!(
            ModelTier::Standard.default_model("anthropic"),
            "claude-sonnet-4-20250514"
        );
        assert_eq!(
            ModelTier::Premium.default_model("anthropic"),
            "claude-opus-4-20250514"
        );

        // Gemini (Gemini 3 family 2026)
        assert_eq!(
            ModelTier::UltraBudget.default_model("gemini"),
            "gemini-3-flash"
        );
        assert_eq!(ModelTier::Fast.default_model("gemini"), "gemini-3-flash");
        assert_eq!(ModelTier::Standard.default_model("gemini"), "gemini-3-pro");

        // DeepSeek (ultra-low-cost)
        assert_eq!(
            ModelTier::UltraBudget.default_model("deepseek"),
            "deepseek-r1-distill-llama-70b"
        );
        assert_eq!(ModelTier::Fast.default_model("deepseek"), "deepseek-chat");
        assert_eq!(
            ModelTier::Premium.default_model("deepseek"),
            "deepseek-reasoner"
        );

        // Groq (FREE - Llama 4)
        assert_eq!(
            ModelTier::UltraBudget.default_model("groq"),
            "llama-4-scout-17b-16e-instruct"
        );
    }

    #[test]
    fn test_model_tier_cost_multiplier() {
        assert_eq!(ModelTier::UltraBudget.cost_multiplier(), 0.1);
        assert_eq!(ModelTier::Fast.cost_multiplier(), 1.0);
        assert_eq!(ModelTier::Standard.cost_multiplier(), 6.0);
        assert_eq!(ModelTier::Premium.cost_multiplier(), 30.0);
    }

    #[test]
    fn test_model_tier_constrain_to() {
        // Premium constrained to Standard should return Standard
        assert_eq!(
            ModelTier::Premium.constrain_to(&ModelTier::Standard),
            ModelTier::Standard
        );
        // Premium constrained to Fast should return Fast
        assert_eq!(
            ModelTier::Premium.constrain_to(&ModelTier::Fast),
            ModelTier::Fast
        );
        // Fast constrained to Premium should stay Fast
        assert_eq!(
            ModelTier::Fast.constrain_to(&ModelTier::Premium),
            ModelTier::Fast
        );
        // UltraBudget constrained to anything should stay UltraBudget
        assert_eq!(
            ModelTier::UltraBudget.constrain_to(&ModelTier::Fast),
            ModelTier::UltraBudget
        );
    }

    #[test]
    fn test_model_tier_price_range() {
        assert_eq!(ModelTier::UltraBudget.price_range(), "< $0.15/M tokens");
        assert_eq!(ModelTier::Fast.price_range(), "$0.15 ~ $1.00/M tokens");
        assert_eq!(ModelTier::Standard.price_range(), "$1.00 ~ $5.00/M tokens");
        assert_eq!(ModelTier::Premium.price_range(), "> $5.00/M tokens");
    }

    #[test]
    fn test_routing_rules_default() {
        let rules = RoutingRules::default();
        assert!(rules.task_providers.is_empty());
        assert!(rules.task_models.is_empty());
        assert!(rules.task_token_budgets.is_empty());
        assert!(!rules.prefer_local);
        assert!(rules.max_tier.is_none());
    }

    #[test]
    fn test_task_type_default_token_budget() {
        // Fast tier tasks should have small budgets
        assert_eq!(
            TaskType::Classification.default_token_budget().max_tokens,
            200
        );
        assert_eq!(TaskType::Extraction.default_token_budget().max_tokens, 500);
        assert_eq!(
            TaskType::Summarization.default_token_budget().max_tokens,
            1000
        );
        assert_eq!(TaskType::Translation.default_token_budget().max_tokens, 800);

        // Standard tier
        assert_eq!(
            TaskType::Conversation.default_token_budget().max_tokens,
            2000
        );

        // Premium tier - larger budgets
        assert_eq!(TaskType::Planning.default_token_budget().max_tokens, 3000);
        assert_eq!(TaskType::CodeReview.default_token_budget().max_tokens, 3000);
        assert_eq!(
            TaskType::CodeGeneration.default_token_budget().max_tokens,
            4096
        );
    }

    #[test]
    fn test_routing_rules_get_token_budget() {
        let mut rules = RoutingRules::default();

        // Should return default budget
        let budget = rules.get_token_budget(TaskType::Classification);
        assert_eq!(budget.max_tokens, 200);

        // With custom override
        rules
            .task_token_budgets
            .insert(TaskType::Classification, TokenBudget::new(500, 0.5));
        let budget = rules.get_token_budget(TaskType::Classification);
        assert_eq!(budget.max_tokens, 500);
        assert_eq!(budget.temperature, 0.5);
    }

    #[test]
    fn test_token_budget_temperatures() {
        // Low temperature for deterministic tasks
        assert!(TaskType::Classification.default_token_budget().temperature < 0.5);
        assert!(TaskType::Extraction.default_token_budget().temperature < 0.5);
        assert!(TaskType::Translation.default_token_budget().temperature < 0.5);

        // Higher temperature for creative tasks
        assert!(TaskType::Conversation.default_token_budget().temperature >= 0.7);
        assert!(TaskType::CodeGeneration.default_token_budget().temperature >= 0.7);
    }

    #[test]
    fn test_router_with_routing_rules() {
        let rules = RoutingRules {
            prefer_local: true,
            max_tier: Some(ModelTier::Standard),
            ..Default::default()
        };

        let router = LlmRouter::new("openai").with_routing_rules(rules);
        assert!(router.routing_rules().prefer_local);
        assert_eq!(router.routing_rules().max_tier, Some(ModelTier::Standard));
    }

    #[test]
    fn test_estimate_cost() {
        let router = LlmRouter::new("openai");

        // UltraBudget tier: 0.1 multiplier
        let ultra_budget_cost = router.estimate_cost(TaskType::Classification, 1000);
        assert_eq!(ultra_budget_cost, 0.1);

        // Fast tier: 1.0 multiplier
        let fast_cost = router.estimate_cost(TaskType::Summarization, 1000);
        assert_eq!(fast_cost, 1.0);

        // Standard tier: 6.0 multiplier
        let standard_cost = router.estimate_cost(TaskType::Conversation, 1000);
        assert_eq!(standard_cost, 6.0);

        // Premium tier: 30.0 multiplier
        let premium_cost = router.estimate_cost(TaskType::CodeGeneration, 1000);
        assert_eq!(premium_cost, 30.0);
    }

    #[test]
    fn test_model_routing_config_get_for_task() {
        let config = ModelRoutingConfig::default();

        // Classification uses UltraBudget -> trivial config
        let trivial = config.get_for_task(TaskType::Classification);
        assert_eq!(trivial.provider, "deepseek");

        // Summarization uses Fast -> simple config
        let simple = config.get_for_task(TaskType::Summarization);
        assert_eq!(simple.provider, "gemini");

        // Conversation uses Standard -> general config
        let general = config.get_for_task(TaskType::Conversation);
        assert_eq!(general.provider, "anthropic");

        // CodeGeneration uses Premium -> complex config
        let complex = config.get_for_task(TaskType::CodeGeneration);
        assert_eq!(complex.provider, "anthropic");
    }

    #[test]
    fn test_model_routing_config_free_tier() {
        let config = ModelRoutingConfig::free_tier();

        // Free tier should use Groq for trivial/simple tasks
        assert_eq!(config.trivial.provider, "groq");
        assert_eq!(config.simple.provider, "groq");
        // And DeepSeek for general/complex tasks
        assert_eq!(config.general.provider, "deepseek");
        assert_eq!(config.complex.provider, "deepseek");
    }

    #[test]
    fn test_model_routing_config_estimate_monthly_cost() {
        let config = ModelRoutingConfig::free_tier();

        // With free tier config (Groq + DeepSeek), cost should be very low
        let cost = config.estimate_monthly_cost(
            1_000_000, // 1M trivial tokens (Groq FREE)
            1_000_000, // 1M simple tokens (Groq FREE)
            1_000_000, // 1M general tokens (DeepSeek $0.21/M)
            100_000,   // 100K complex tokens (DeepSeek $1.37/M)
        );

        // Expected: 0 + 0 + 0.21 + 0.137 = ~$0.35
        assert!(cost < 1.0, "Free tier should be under $1 for 3.1M tokens");
    }
}
