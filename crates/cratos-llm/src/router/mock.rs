//! Mock LLM Provider for testing
//!
//! This module provides a mock provider that returns empty responses.

use super::provider::LlmProvider;
use crate::completion::{
    CompletionRequest, CompletionResponse, ToolCompletionRequest, ToolCompletionResponse,
};
use crate::error::Result;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A mock LLM provider that returns queued responses or default empty ones.
pub struct MockProvider {
    responses: Arc<Mutex<VecDeque<ToolCompletionResponse>>>,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockProvider {
    /// Create a new mock provider.
    #[must_use]
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Add a response to the queue.
    pub fn add_tool_response(&self, response: ToolCompletionResponse) {
        self.responses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(response);
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
        // Simple default for simple complete
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
        let mut responses = self.responses.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(resp) = responses.pop_front() {
            Ok(resp)
        } else {
            // Default behavior if queue empty
            Ok(ToolCompletionResponse {
                content: Some("mock response".to_string()),
                tool_calls: vec![],
                usage: None,
                finish_reason: Some("stop".to_string()),
                model: "mock-model".to_string(),
            })
        }
    }
}
