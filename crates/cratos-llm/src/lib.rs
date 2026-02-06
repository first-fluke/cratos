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

pub mod anthropic;
pub mod cli_auth;
pub mod completion;
pub mod cost;
pub mod deepseek;
#[cfg(feature = "embeddings")]
pub mod embeddings;
pub mod error;
pub mod fireworks;
pub mod gemini;
pub mod glm;
pub mod groq;
pub mod message;
pub mod moonshot;
pub mod novita;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod quota;
pub mod qwen;
pub mod router;
pub mod siliconflow;
pub mod oauth;
pub mod oauth_config;
pub mod token;
pub mod tools;
pub mod util;

pub use cost::{
    global_tracker, CostReport, CostTracker, ModelPricing, SavingsPotential, UsageRecord,
    UsageStats,
};
pub use quota::{
    format_compact_number, format_duration, global_quota_tracker, QuotaState, QuotaTracker,
};
pub use error::{Error, Result};
pub use router::{
    count_message_tokens, count_tokens, CompletionRequest, CompletionResponse, LlmProvider,
    LlmRouter, Message, MessageRole, ModelConfig, ModelRoutingConfig, ModelTier, ProviderConfig,
    RouterConfig, RoutingRules, TaskType, TokenBudget, TokenCounter, TokenUsage, ToolCall,
    ToolChoice, ToolCompletionRequest, ToolCompletionResponse, ToolDefinition, TOKEN_COUNTER,
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
