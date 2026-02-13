//! LLM Router implementation
//!
//! This module contains the main LlmRouter struct that manages multiple providers
//! and provides intelligent routing based on task types.

use super::rules::RoutingRules;
use super::provider::LlmProvider;
use super::types::TaskType;
use crate::completion::{
    CompletionRequest, CompletionResponse, ToolCompletionRequest, ToolCompletionResponse,
};
use crate::error::{Error, Result};
use crate::tools::ToolChoice;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument};

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
        let provider = self.default_provider().ok_or_else(|| {
            if self.default_provider == "auto" {
                Error::NotConfigured(
                    "auto (no provider resolved — check API keys or run `cratos init`)".into(),
                )
            } else {
                Error::NotConfigured(self.default_provider.clone())
            }
        })?;

        provider.complete(request).await
    }

    /// Complete with tools using the default provider
    #[instrument(skip(self, request))]
    pub async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse> {
        let provider = self.default_provider().ok_or_else(|| {
            if self.default_provider == "auto" {
                Error::NotConfigured(
                    "auto (no provider resolved — check API keys or run `cratos init`)".into(),
                )
            } else {
                Error::NotConfigured(self.default_provider.clone())
            }
        })?;

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
        messages: Vec<crate::message::Message>,
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
        messages: Vec<crate::message::Message>,
        tools: Vec<crate::tools::ToolDefinition>,
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
        self.providers
            .get(&self.default_provider)
            .map(|p| p.default_model())
            .unwrap_or(&self.default_provider)
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
