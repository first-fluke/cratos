//! Mock LLM Provider for testing
//!
//! This module provides a mock provider that returns empty responses.

use super::provider::LlmProvider;
use crate::completion::{
    CompletionRequest, CompletionResponse, ToolCompletionRequest, ToolCompletionResponse,
};
use crate::error::Result;

/// A mock LLM provider that returns empty responses. Useful for testing.
pub struct MockProvider;

impl Default for MockProvider {
    fn default() -> Self {
        Self
    }
}

impl MockProvider {
    /// Create a new mock provider.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn available_models(&self) -> Vec<String> {
        vec!["mock-model".to_string()]
    }

    fn default_model(&self) -> &str {
        "mock-model"
    }

    async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse> {
        Ok(CompletionResponse {
            content: "mock response".to_string(),
            usage: None,
            finish_reason: Some("stop".to_string()),
            model: "mock-model".to_string(),
        })
    }

    async fn complete_with_tools(
        &self,
        _request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse> {
        Ok(ToolCompletionResponse {
            content: Some("mock response".to_string()),
            tool_calls: vec![],
            usage: None,
            finish_reason: Some("stop".to_string()),
            model: "mock-model".to_string(),
        })
    }
}
