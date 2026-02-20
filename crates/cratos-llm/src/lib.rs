//! Cratos LLM - LLM Provider Abstraction
//!
//! This crate provides LLM integration for Cratos:
//! - Router: Provider trait definition and automatic routing
//! - OpenAI: GPT-5 family (5, 5-nano) + GPT-4o (legacy)
//! - Anthropic: Claude 4 family (Haiku, Sonnet 4, Opus 4.5)
//! - Gemini: Google Gemini 2.5 family (Flash, Pro)
//! - Ollama: Local Ollama provider (Qwen 2.5, Llama 3.1)
//! - GLM: ZhipuAI GLM-4 provider
//! - Qwen: Alibaba Qwen provider
//! - OpenRouter: Multi-provider gateway
//! - Novita: Free tier LLM provider
//! - Groq: Free tier with Llama 3.3 (ultra-fast inference)
//! - DeepSeek: Ultra-low-cost provider ($0.03 ~ $0.55/1M tokens)
//! - SiliconFlow: Cheapest provider ($0.03 ~ $0.09/1M tokens)
//! - Fireworks: Fast inference for open-source models
//! - Embeddings: Vector embeddings for semantic search (feature: embeddings)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// LLM API providers
pub mod providers;

pub use providers::anthropic;
pub mod cli_auth;
pub mod completion;
pub mod cost;
pub use providers::deepseek;
#[cfg(feature = "embeddings")]
pub mod embeddings;
pub mod error;
pub use providers::fireworks;
pub use providers::gemini;
/// Gemini CLI credential extraction module.
pub use providers::gemini_auth;
pub use providers::glm;
pub use providers::groq;
pub mod message;
pub use providers::moonshot;
pub use providers::novita;
pub mod oauth;
pub mod oauth_config;
pub use providers::ollama;
pub use providers::openai;
pub use providers::openrouter;
pub mod quota;
pub use providers::qwen;
pub mod router;
pub use providers::siliconflow;
pub mod token;
pub mod tools;
pub mod util;

pub use cost::{
    global_tracker, CostReport, CostTracker, ModelPricing, SavingsPotential, UsageRecord,
    UsageStats,
};
pub use error::{Error, Result};
pub use gemini_quota::start_gemini_quota_poller;
pub use providers::gemini_quota;
pub use quota::{
    format_compact_number, format_duration, global_quota_tracker, QuotaSource, QuotaState,
    QuotaTracker,
};
pub use router::{
    count_message_tokens, count_tokens, CompletionRequest, CompletionResponse, ImageContent,
    LlmProvider, LlmRouter, Message, MessageRole, MockProvider, ModelConfig, ModelRoutingConfig,
    ModelTier, ProviderConfig, RouterConfig, RoutingRules, TaskType, TokenBudget, TokenCounter,
    TokenUsage, ToolCall, ToolChoice, ToolCompletionRequest, ToolCompletionResponse,
    ToolDefinition, TOKEN_COUNTER,
};

// Re-export provider types
pub use anthropic::{AnthropicConfig, AnthropicProvider};
pub use deepseek::{DeepSeekConfig, DeepSeekProvider};
pub use fireworks::{FireworksConfig, FireworksProvider};
pub use gemini::{GeminiConfig, GeminiProvider};
pub use glm::{GlmConfig, GlmProvider};
pub use groq::{GroqConfig, GroqProvider};
pub use moonshot::{MoonshotConfig, MoonshotProvider};
pub use novita::{NovitaConfig, NovitaProvider};
pub use ollama::{OllamaConfig, OllamaProvider};
pub use openai::{OpenAiConfig, OpenAiProvider};
pub use openrouter::{OpenRouterConfig, OpenRouterProvider};
pub use qwen::{QwenConfig, QwenProvider};
pub use siliconflow::{SiliconFlowConfig, SiliconFlowProvider};

// Re-export embeddings (when feature is enabled)
#[cfg(feature = "embeddings")]
pub use embeddings::{
    default_embedding_provider, EmbeddingProvider, SharedEmbeddingProvider, TractEmbeddingProvider,
};
