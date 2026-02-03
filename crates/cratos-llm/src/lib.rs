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

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod anthropic;
pub mod cost;
pub mod error;
pub mod gemini;
pub mod glm;
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
    LlmRouter, Message, MessageRole, ModelTier, ProviderConfig, RouterConfig, RoutingRules,
    TaskType, TokenBudget, TokenCounter, TokenUsage, ToolCall, ToolChoice, ToolCompletionRequest,
    ToolCompletionResponse, ToolDefinition, TOKEN_COUNTER,
};

// Re-export provider types
pub use anthropic::{AnthropicConfig, AnthropicProvider};
pub use gemini::{GeminiConfig, GeminiProvider};
pub use glm::{GlmConfig, GlmProvider};
pub use novita::{NovitaConfig, NovitaProvider};
pub use ollama::{OllamaConfig, OllamaProvider};
pub use openai::{OpenAiConfig, OpenAiProvider};
pub use openrouter::{OpenRouterConfig, OpenRouterProvider};
pub use qwen::{QwenConfig, QwenProvider};
