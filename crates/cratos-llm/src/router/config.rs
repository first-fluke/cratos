//! Configuration types for LLM routing
//!
//! This module contains configuration structs for providers and model routing.

use super::types::{ModelTier, TaskType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Provider Configuration
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

// ============================================================================
// Model Configuration
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

// ============================================================================
// Model Routing Configuration
// ============================================================================

fn default_true() -> bool {
    true
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

impl Default for ModelRoutingConfig {
    fn default() -> Self {
        Self {
            // UltraBudget - DeepSeek R1 Distill ($0.03/1M input, $0.09/1M output)
            trivial: ModelConfig::new("deepseek", "deepseek-r1-distill-llama-70b"),
            // Fast - Gemini 2.5 Flash ($0.075/1M input, free tier available)
            simple: ModelConfig::new("gemini", "gemini-2.5-flash"),
            // Standard - Claude Sonnet 4.5 ($3/1M, balanced)
            general: ModelConfig::new("anthropic", "claude-sonnet-4-5-20250929"),
            // Premium - Claude Opus 4.5 ($5/$25, 67% cheaper than Opus 4!)
            complex: ModelConfig::new("anthropic", "claude-opus-4-5-20250514"),
            // Fallback to GPT-5-nano ($0.05/$0.40 — cheaper than GPT-4o-mini)
            fallback: Some(ModelConfig::new("openai", "gpt-5-nano")),
            auto_downgrade: true,
        }
    }
}

impl ModelRoutingConfig {
    /// Create a config optimized for free/low-cost usage
    ///
    /// Uses ultra-low-cost providers (Groq, DeepSeek):
    /// - Trivial: Groq Llama 3.1 8B ($0.05/$0.08)
    /// - Simple: Groq GPT-OSS 20B ($0.075/$0.30, tool use support)
    /// - General: DeepSeek Chat ($0.14/$0.28)
    /// - Complex: DeepSeek Reasoner ($0.55/$2.19)
    #[must_use]
    pub fn free_tier() -> Self {
        Self {
            trivial: ModelConfig::new("groq", "llama-3.1-8b-instant"),
            simple: ModelConfig::new("groq", "openai/gpt-oss-20b"),
            general: ModelConfig::new("deepseek", "deepseek-chat"),
            complex: ModelConfig::new("deepseek", "deepseek-reasoner"),
            fallback: Some(ModelConfig::new("novita", "qwen/qwen2.5-72b-instruct")),
            auto_downgrade: true,
        }
    }

    /// Create a config optimized for quality (uses premium models)
    ///
    /// Uses Anthropic Claude 4.5 family for all tiers:
    /// - Trivial: Claude Haiku 4.5 ($1/$5)
    /// - Simple: Claude Haiku 4.5 ($1/$5)
    /// - General: Claude Sonnet 4.5 ($3/$15)
    /// - Complex: Claude Opus 4.5 ($5/$25)
    #[must_use]
    pub fn quality_tier() -> Self {
        Self {
            trivial: ModelConfig::new("anthropic", "claude-haiku-4-5-20251001"),
            simple: ModelConfig::new("anthropic", "claude-haiku-4-5-20251001"),
            general: ModelConfig::new("anthropic", "claude-sonnet-4-5-20250929"),
            complex: ModelConfig::new("anthropic", "claude-opus-4-5-20250514"),
            fallback: Some(ModelConfig::new("openai", "gpt-5")),
            auto_downgrade: false,
        }
    }

    /// Create a config for local-only usage (Ollama)
    ///
    /// All tiers use the same local model (Qwen 2.5 7B)
    #[must_use]
    pub fn local_only() -> Self {
        Self {
            trivial: ModelConfig::new("ollama", "qwen2.5:7b"),
            simple: ModelConfig::new("ollama", "qwen2.5:7b"),
            general: ModelConfig::new("ollama", "qwen2.5:7b"),
            complex: ModelConfig::new("ollama", "qwen2.5:7b"),
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
            // Groq - low-cost (free tier available with rate limits)
            ("groq", ModelTier::UltraBudget) => 0.065, // llama-3.1-8b
            ("groq", ModelTier::Fast) => 0.19,         // gpt-oss-20b
            ("groq", ModelTier::Standard) => 0.375,    // gpt-oss-120b
            ("groq", ModelTier::Premium) => 0.69,      // llama-3.3-70b
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
            ("openai", ModelTier::Standard) => 5.63, // GPT-5 avg
            ("openai", ModelTier::Premium) => 5.63,
            // Anthropic - Claude 4.5 family
            ("anthropic", ModelTier::UltraBudget) => 3.0, // Haiku 4.5 avg
            ("anthropic", ModelTier::Fast) => 3.0,
            ("anthropic", ModelTier::Standard) => 9.0, // Sonnet 4.5 avg
            ("anthropic", ModelTier::Premium) => 15.0, // Opus 4.5 avg
            // Gemini - Gemini 2.5 family
            ("gemini" | "google_pro", ModelTier::UltraBudget) => 0.175, // Flash avg
            ("gemini" | "google_pro", ModelTier::Fast) => 0.175,
            ("gemini" | "google_pro", ModelTier::Standard) => 1.25, // Pro avg
            ("gemini" | "google_pro", ModelTier::Premium) => 1.25,
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
