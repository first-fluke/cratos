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
//!
//! # Module Structure
//!
//! - `types`: Core types (TaskType, ModelTier)
//! - `rules`: Routing rules configuration
//! - `config`: Provider and model configuration
//! - `provider`: LlmProvider trait definition
//! - `mock`: Mock provider for testing
//! - `router_impl`: LlmRouter implementation

mod config;
mod mock;
mod provider;
mod router_impl;
mod rules;
mod types;

#[cfg(test)]
mod tests;

// Re-export types from submodules for backward compatibility
pub use crate::completion::{
    CompletionRequest, CompletionResponse, TokenUsage, ToolCompletionRequest,
    ToolCompletionResponse,
};
pub use crate::message::{ImageContent, Message, MessageRole};
pub use crate::token::{
    count_message_tokens, count_tokens, TokenBudget, TokenCounter, TOKEN_COUNTER,
};
pub use crate::tools::{ToolCall, ToolChoice, ToolDefinition};

// Re-export from this module's submodules
pub use config::{ModelConfig, ModelRoutingConfig, ProviderConfig, RouterConfig};
pub use mock::MockProvider;
pub use provider::LlmProvider;
pub use router_impl::LlmRouter;
pub use rules::RoutingRules;
pub use types::{ModelTier, TaskType};
