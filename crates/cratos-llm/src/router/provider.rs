//! LLM Provider trait definition
//!
//! This module defines the core trait that all LLM providers must implement.

use crate::completion::{
    CompletionRequest, CompletionResponse, ToolCompletionRequest, ToolCompletionResponse,
};
use crate::error::Result;

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
