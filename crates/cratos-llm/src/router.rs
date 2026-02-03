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
use tiktoken_rs::{cl100k_base, CoreBPE};
use tracing::{debug, info, instrument};

// ============================================================================
// Token Counting
// ============================================================================

/// Token counter for estimating message token usage
///
/// Uses tiktoken's cl100k_base encoding (GPT-4, Claude 3, etc.)
/// for accurate token estimation across modern LLMs.
pub struct TokenCounter {
    bpe: CoreBPE,
}

impl TokenCounter {
    /// Create a new token counter
    ///
    /// Uses cl100k_base encoding which is compatible with:
    /// - OpenAI GPT-4, GPT-4o, GPT-3.5-turbo
    /// - Anthropic Claude 3.x (approximate)
    /// - Most modern LLMs
    #[must_use]
    pub fn new() -> Self {
        Self {
            bpe: cl100k_base().expect("Failed to load cl100k_base tokenizer"),
        }
    }

    /// Count tokens in a string
    #[must_use]
    pub fn count_tokens(&self, text: &str) -> usize {
        self.bpe.encode_with_special_tokens(text).len()
    }

    /// Count tokens in a message (includes role overhead)
    ///
    /// Adds overhead for message structure:
    /// - Role marker: ~4 tokens
    /// - Message separators: ~2 tokens
    #[must_use]
    pub fn count_message_tokens(&self, message: &Message) -> usize {
        const MESSAGE_OVERHEAD: usize = 6; // role + separators
        self.count_tokens(&message.content) + MESSAGE_OVERHEAD
    }

    /// Count total tokens in a conversation
    #[must_use]
    pub fn count_conversation_tokens(&self, messages: &[Message]) -> usize {
        const CONVERSATION_OVERHEAD: usize = 3; // start/end tokens
        messages
            .iter()
            .map(|m| self.count_message_tokens(m))
            .sum::<usize>()
            + CONVERSATION_OVERHEAD
    }

    /// Estimate tokens for a tool definition
    #[must_use]
    pub fn count_tool_tokens(&self, tool: &ToolDefinition) -> usize {
        const TOOL_OVERHEAD: usize = 10; // structure overhead
        self.count_tokens(&tool.name)
            + self.count_tokens(&tool.description)
            + self.count_tokens(&tool.parameters.to_string())
            + TOOL_OVERHEAD
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

// Thread-safe global token counter
lazy_static::lazy_static! {
    /// Global token counter instance for convenience
    pub static ref TOKEN_COUNTER: TokenCounter = TokenCounter::new();
}

/// Convenience function to count tokens in text
#[must_use]
pub fn count_tokens(text: &str) -> usize {
    TOKEN_COUNTER.count_tokens(text)
}

/// Convenience function to count tokens in messages
#[must_use]
pub fn count_message_tokens(messages: &[Message]) -> usize {
    TOKEN_COUNTER.count_conversation_tokens(messages)
}

// ============================================================================
// Core Types
// ============================================================================

/// Role in a conversation message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message (instructions)
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool response
    Tool,
}

impl MessageRole {
    /// Returns the string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// Tool call ID (for tool responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Name (for tool calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    /// Create a system message
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }

    /// Create a user message
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }

    /// Create an assistant message
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }

    /// Create a tool response message
    #[must_use]
    pub fn tool_response(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            name: None,
        }
    }
}

/// Tool definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON schema for parameters
    pub parameters: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new tool definition
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// A tool call requested by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call
    pub id: String,
    /// Tool name
    pub name: String,
    /// Arguments as JSON string
    pub arguments: String,
}

impl ToolCall {
    /// Parse arguments as a typed value
    pub fn parse_arguments<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.arguments).map_err(|e| Error::InvalidResponse(e.to_string()))
    }
}

/// Token usage information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Prompt tokens
    pub prompt_tokens: u32,
    /// Completion tokens
    pub completion_tokens: u32,
    /// Total tokens
    pub total_tokens: u32,
}

/// Completion request
#[derive(Debug, Clone, Default)]
pub struct CompletionRequest {
    /// Model to use (provider-specific)
    pub model: String,
    /// Messages in the conversation
    pub messages: Vec<Message>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature (0.0 - 2.0)
    pub temperature: Option<f32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
}

impl CompletionRequest {
    /// Create a new completion request
    #[must_use]
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Add a message
    #[must_use]
    pub fn with_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    /// Add messages
    #[must_use]
    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages.extend(messages);
        self
    }

    /// Set max tokens
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

/// Completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Generated content
    pub content: String,
    /// Token usage
    pub usage: Option<TokenUsage>,
    /// Finish reason
    pub finish_reason: Option<String>,
    /// Model used
    pub model: String,
}

/// Request with tools
#[derive(Debug, Clone)]
pub struct ToolCompletionRequest {
    /// Base completion request
    pub request: CompletionRequest,
    /// Available tools
    pub tools: Vec<ToolDefinition>,
    /// Tool choice strategy
    pub tool_choice: ToolChoice,
}

impl ToolCompletionRequest {
    /// Create a new tool completion request
    #[must_use]
    pub fn new(request: CompletionRequest, tools: Vec<ToolDefinition>) -> Self {
        Self {
            request,
            tools,
            tool_choice: ToolChoice::Auto,
        }
    }

    /// Set tool choice
    #[must_use]
    pub fn with_tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = choice;
        self
    }
}

/// Tool choice strategy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoice {
    /// Let the model decide
    #[default]
    Auto,
    /// Don't use tools
    None,
    /// Force a specific tool
    Required,
    /// Use a specific tool by name
    Tool(String),
}

/// Response that may include tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCompletionResponse {
    /// Text content (if any)
    pub content: Option<String>,
    /// Tool calls requested
    pub tool_calls: Vec<ToolCall>,
    /// Token usage
    pub usage: Option<TokenUsage>,
    /// Finish reason
    pub finish_reason: Option<String>,
    /// Model used
    pub model: String,
}

impl ToolCompletionResponse {
    /// Check if the response has tool calls
    #[must_use]
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

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

/// Token budget configuration for different task types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Maximum tokens to generate for this task type
    pub max_tokens: u32,
    /// Recommended temperature for this task type
    pub temperature: f32,
}

impl TokenBudget {
    /// Create a new token budget
    #[must_use]
    pub const fn new(max_tokens: u32, temperature: f32) -> Self {
        Self {
            max_tokens,
            temperature,
        }
    }
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.7,
        }
    }
}

impl TaskType {
    /// Get the recommended model tier for this task type
    #[must_use]
    pub fn recommended_tier(&self) -> ModelTier {
        match self {
            Self::Classification => ModelTier::Fast,
            Self::Summarization => ModelTier::Fast,
            Self::Extraction => ModelTier::Fast,
            Self::Translation => ModelTier::Fast,
            Self::Conversation => ModelTier::Standard,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    /// Fast, cheap models for simple tasks (GPT-4o-mini, Claude Haiku, Gemini Flash)
    Fast,
    /// Balanced models for general tasks (GPT-4o, Claude Sonnet)
    Standard,
    /// Premium models for complex reasoning (GPT-4, Claude Opus)
    Premium,
}

impl ModelTier {
    /// Get default model for each provider at this tier
    #[must_use]
    pub fn default_model(&self, provider: &str) -> &'static str {
        match (self, provider) {
            // OpenAI models
            (ModelTier::Fast, "openai") => "gpt-4o-mini",
            (ModelTier::Standard, "openai") => "gpt-4o",
            (ModelTier::Premium, "openai") => "gpt-4-turbo",
            // Anthropic models
            (ModelTier::Fast, "anthropic") => "claude-3-5-haiku-20241022",
            (ModelTier::Standard, "anthropic") => "claude-3-5-sonnet-20241022",
            (ModelTier::Premium, "anthropic") => "claude-3-opus-20240229",
            // Gemini models
            (ModelTier::Fast, "gemini") => "gemini-1.5-flash",
            (ModelTier::Standard, "gemini") => "gemini-1.5-pro",
            (ModelTier::Premium, "gemini") => "gemini-1.5-pro",
            // GLM models
            (ModelTier::Fast, "glm") => "glm-4-flash",
            (ModelTier::Standard, "glm") => "glm-4-9b",
            (ModelTier::Premium, "glm") => "glm-4-plus",
            // Qwen models
            (ModelTier::Fast, "qwen") => "qwen-turbo",
            (ModelTier::Standard, "qwen") => "qwen-plus",
            (ModelTier::Premium, "qwen") => "qwen-max",
            // OpenRouter models
            (ModelTier::Fast, "openrouter") => "qwen/qwen3-32b:free",
            (ModelTier::Standard, "openrouter") => "openai/gpt-4o-mini",
            (ModelTier::Premium, "openrouter") => "anthropic/claude-3.5-sonnet",
            // Novita models (free tier)
            (ModelTier::Fast, "novita") => "qwen/qwen2.5-7b-instruct",
            (ModelTier::Standard, "novita") => "thudm/glm-4-9b-chat",
            (ModelTier::Premium, "novita") => "qwen/qwen2.5-72b-instruct",
            // Ollama (local) - all same tier
            (_, "ollama") => "llama3.2",
            // Default
            _ => "gpt-4o",
        }
    }

    /// Estimated cost multiplier relative to Fast tier
    #[must_use]
    pub fn cost_multiplier(&self) -> f32 {
        match self {
            ModelTier::Fast => 1.0,
            ModelTier::Standard => 5.0,
            ModelTier::Premium => 20.0,
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

        // Apply max tier constraint
        if let Some(max_tier) = &self.routing_rules.max_tier {
            tier = match (tier, max_tier) {
                (ModelTier::Premium, ModelTier::Fast) => ModelTier::Fast,
                (ModelTier::Premium, ModelTier::Standard) => ModelTier::Standard,
                (ModelTier::Standard, ModelTier::Fast) => ModelTier::Fast,
                _ => tier,
            };
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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let system = Message::system("You are a helpful assistant");
        assert_eq!(system.role, MessageRole::System);

        let user = Message::user("Hello!");
        assert_eq!(user.role, MessageRole::User);

        let assistant = Message::assistant("Hi there!");
        assert_eq!(assistant.role, MessageRole::Assistant);

        let tool = Message::tool_response("call_123", r#"{"result": "ok"}"#);
        assert_eq!(tool.role, MessageRole::Tool);
        assert_eq!(tool.tool_call_id, Some("call_123".to_string()));
    }

    #[test]
    fn test_completion_request_builder() {
        let request = CompletionRequest::new("gpt-4")
            .with_message(Message::system("You are helpful"))
            .with_message(Message::user("Hello"))
            .with_max_tokens(100)
            .with_temperature(0.7);

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.max_tokens, Some(100));
        assert_eq!(request.temperature, Some(0.7));
    }

    #[test]
    fn test_tool_definition() {
        let tool = ToolDefinition::new(
            "get_weather",
            "Get the current weather",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }),
        );

        assert_eq!(tool.name, "get_weather");
    }

    #[test]
    fn test_router() {
        let router = LlmRouter::new("openai");
        assert_eq!(router.default_provider_name(), "openai");
        assert!(!router.has_provider("openai"));
        assert!(router.list_providers().is_empty());
    }

    #[test]
    fn test_task_type_recommended_tier() {
        // Fast tier tasks
        assert_eq!(TaskType::Classification.recommended_tier(), ModelTier::Fast);
        assert_eq!(TaskType::Summarization.recommended_tier(), ModelTier::Fast);
        assert_eq!(TaskType::Extraction.recommended_tier(), ModelTier::Fast);

        // Standard tier tasks
        assert_eq!(
            TaskType::Conversation.recommended_tier(),
            ModelTier::Standard
        );

        // Premium tier tasks
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
        // OpenAI
        assert_eq!(ModelTier::Fast.default_model("openai"), "gpt-4o-mini");
        assert_eq!(ModelTier::Standard.default_model("openai"), "gpt-4o");
        assert_eq!(ModelTier::Premium.default_model("openai"), "gpt-4-turbo");

        // Anthropic
        assert_eq!(
            ModelTier::Fast.default_model("anthropic"),
            "claude-3-5-haiku-20241022"
        );
        assert_eq!(
            ModelTier::Standard.default_model("anthropic"),
            "claude-3-5-sonnet-20241022"
        );
        assert_eq!(
            ModelTier::Premium.default_model("anthropic"),
            "claude-3-opus-20240229"
        );

        // Gemini
        assert_eq!(ModelTier::Fast.default_model("gemini"), "gemini-1.5-flash");
        assert_eq!(
            ModelTier::Standard.default_model("gemini"),
            "gemini-1.5-pro"
        );
    }

    #[test]
    fn test_model_tier_cost_multiplier() {
        assert_eq!(ModelTier::Fast.cost_multiplier(), 1.0);
        assert_eq!(ModelTier::Standard.cost_multiplier(), 5.0);
        assert_eq!(ModelTier::Premium.cost_multiplier(), 20.0);
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
        assert_eq!(TaskType::Classification.default_token_budget().max_tokens, 200);
        assert_eq!(TaskType::Extraction.default_token_budget().max_tokens, 500);
        assert_eq!(TaskType::Summarization.default_token_budget().max_tokens, 1000);
        assert_eq!(TaskType::Translation.default_token_budget().max_tokens, 800);

        // Standard tier
        assert_eq!(TaskType::Conversation.default_token_budget().max_tokens, 2000);

        // Premium tier - larger budgets
        assert_eq!(TaskType::Planning.default_token_budget().max_tokens, 3000);
        assert_eq!(TaskType::CodeReview.default_token_budget().max_tokens, 3000);
        assert_eq!(TaskType::CodeGeneration.default_token_budget().max_tokens, 4096);
    }

    #[test]
    fn test_routing_rules_get_token_budget() {
        let mut rules = RoutingRules::default();

        // Should return default budget
        let budget = rules.get_token_budget(TaskType::Classification);
        assert_eq!(budget.max_tokens, 200);

        // With custom override
        rules.task_token_budgets.insert(
            TaskType::Classification,
            TokenBudget::new(500, 0.5),
        );
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

        // Fast tier: 1.0 multiplier
        let fast_cost = router.estimate_cost(TaskType::Classification, 1000);
        assert_eq!(fast_cost, 1.0);

        // Standard tier: 5.0 multiplier
        let standard_cost = router.estimate_cost(TaskType::Conversation, 1000);
        assert_eq!(standard_cost, 5.0);

        // Premium tier: 20.0 multiplier
        let premium_cost = router.estimate_cost(TaskType::CodeGeneration, 1000);
        assert_eq!(premium_cost, 20.0);
    }

    // ========================================================================
    // Token Counter Tests
    // ========================================================================

    #[test]
    fn test_token_counter_basic() {
        let counter = TokenCounter::new();

        // Simple text
        let tokens = counter.count_tokens("Hello, world!");
        assert!(tokens > 0);
        assert!(tokens < 10); // Should be ~4 tokens

        // Empty string
        assert_eq!(counter.count_tokens(""), 0);
    }

    #[test]
    fn test_token_counter_message() {
        let counter = TokenCounter::new();

        let message = Message::user("Hello, how are you?");
        let tokens = counter.count_message_tokens(&message);

        // Should include content + overhead
        let content_tokens = counter.count_tokens("Hello, how are you?");
        assert!(tokens > content_tokens);
    }

    #[test]
    fn test_token_counter_conversation() {
        let counter = TokenCounter::new();

        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello!"),
            Message::assistant("Hi there! How can I help you?"),
        ];

        let total = counter.count_conversation_tokens(&messages);

        // Should be sum of messages + overhead
        let sum: usize = messages.iter().map(|m| counter.count_message_tokens(m)).sum();
        assert!(total >= sum);
    }

    #[test]
    fn test_token_counter_tool() {
        let counter = TokenCounter::new();

        let tool = ToolDefinition::new(
            "get_weather",
            "Get the current weather for a location",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string", "description": "City name"}
                },
                "required": ["location"]
            }),
        );

        let tokens = counter.count_tool_tokens(&tool);
        assert!(tokens > 0);
    }

    #[test]
    fn test_global_token_counter() {
        // Test convenience functions
        let tokens = count_tokens("Hello, world!");
        assert!(tokens > 0);

        let messages = vec![Message::user("Hello!")];
        let msg_tokens = count_message_tokens(&messages);
        assert!(msg_tokens > tokens); // Should include overhead
    }
}
