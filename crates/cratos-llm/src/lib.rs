//! Cratos LLM - LLM Provider Abstraction
//!
//! This crate provides LLM integration for Cratos:
//! - Router: Provider trait definition and automatic routing
//! - OpenAI: async-openai provider
//! - Anthropic: Claude API provider
//! - Gemini: Google Gemini API provider
//! - Ollama: Local Ollama provider

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod anthropic;
pub mod cost;
pub mod error;
pub mod gemini;
pub mod ollama;
pub mod openai;
pub mod router;

pub use cost::{
    global_tracker, CostReport, CostTracker, ModelPricing, SavingsPotential, UsageRecord,
    UsageStats,
};
pub use error::{Error, Result};
pub use router::{
    CompletionRequest, CompletionResponse, LlmProvider, LlmRouter, Message, MessageRole, ModelTier,
    ProviderConfig, RouterConfig, RoutingRules, TaskType, TokenUsage, ToolCall, ToolChoice,
    ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
};

// Re-export provider types
pub use anthropic::{AnthropicConfig, AnthropicProvider};
pub use gemini::{GeminiConfig, GeminiProvider};
pub use ollama::{OllamaConfig, OllamaProvider};
pub use openai::{OpenAiConfig, OpenAiProvider};
