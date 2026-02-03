//! Cratos LLM - LLM Provider Abstraction
//!
//! This crate provides LLM integration for Cratos:
//! - Router: Provider trait definition and automatic routing
//! - OpenAI: async-openai provider
//! - Anthropic: Claude API provider
//! - Gemini: Google Gemini API provider
//! - Ollama: Local Ollama provider
//! - GLM: ZhipuAI GLM provider
//! - Qwen: Alibaba Qwen provider
//! - OpenRouter: Multi-provider gateway
//! - Novita: Free tier LLM provider
//! - Groq: Free tier with ultra-fast inference
//! - DeepSeek: Ultra-low-cost provider ($0.14/1M tokens)
//! - Embeddings: Vector embeddings for semantic search (feature: embeddings)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod anthropic;
pub mod cost;
pub mod deepseek;
#[cfg(feature = "embeddings")]
pub mod embeddings;
pub mod error;
pub mod gemini;
pub mod glm;
pub mod groq;
pub mod novita;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod qwen;
pub mod router;

pub use cost::{
    global_tracker, CostReport, CostTracker, ModelPricing, SavingsPotential, UsageRecord,
    UsageStats,
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
pub use gemini::{GeminiConfig, GeminiProvider};
pub use glm::{GlmConfig, GlmProvider};
pub use groq::{GroqConfig, GroqProvider};
pub use novita::{NovitaConfig, NovitaProvider};
pub use ollama::{OllamaConfig, OllamaProvider};
pub use openai::{OpenAiConfig, OpenAiProvider};
pub use openrouter::{OpenRouterConfig, OpenRouterProvider};
pub use qwen::{QwenConfig, QwenProvider};

// Re-export embeddings (when feature is enabled)
#[cfg(feature = "embeddings")]
pub use embeddings::{
    default_embedding_provider, EmbeddingProvider, FastEmbedProvider, SharedEmbeddingProvider,
};
