//! Completion request and response types
//!
//! This module defines the types for LLM completion requests and responses.

use crate::message::Message;
use crate::tools::{ToolCall, ToolChoice, ToolDefinition};
use serde::{Deserialize, Serialize};

/// Token usage information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Prompt tokens
    pub prompt_tokens: u32,
    /// Completion tokens
    pub completion_tokens: u32,
    /// Total tokens
    pub total_tokens: u32,
}

/// Completion request
#[derive(Debug, Clone, Default)]
pub struct CompletionRequest {
    /// Model to use (provider-specific)
    pub model: String,
    /// Messages in the conversation
    pub messages: Vec<Message>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature (0.0 - 2.0)
    pub temperature: Option<f32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
}

impl CompletionRequest {
    /// Create a new completion request
    #[must_use]
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Add a message
    #[must_use]
    pub fn with_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    /// Add messages
    #[must_use]
    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages.extend(messages);
        self
    }

    /// Set max tokens
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

/// Completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Generated content
    pub content: String,
    /// Token usage
    pub usage: Option<TokenUsage>,
    /// Finish reason
    pub finish_reason: Option<String>,
    /// Model used
    pub model: String,
}

/// Request with tools
#[derive(Debug, Clone)]
pub struct ToolCompletionRequest {
    /// Base completion request
    pub request: CompletionRequest,
    /// Available tools
    pub tools: Vec<ToolDefinition>,
    /// Tool choice strategy
    pub tool_choice: ToolChoice,
}

impl ToolCompletionRequest {
    /// Create a new tool completion request
    #[must_use]
    pub fn new(request: CompletionRequest, tools: Vec<ToolDefinition>) -> Self {
        Self {
            request,
            tools,
            tool_choice: ToolChoice::Auto,
        }
    }

    /// Set tool choice
    #[must_use]
    pub fn with_tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = choice;
        self
    }
}

/// Response that may include tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCompletionResponse {
    /// Text content (if any)
    pub content: Option<String>,
    /// Tool calls requested
    pub tool_calls: Vec<ToolCall>,
    /// Token usage
    pub usage: Option<TokenUsage>,
    /// Finish reason
    pub finish_reason: Option<String>,
    /// Model used
    pub model: String,
}

impl ToolCompletionResponse {
    /// Check if the response has tool calls
    #[must_use]
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_request_builder() {
        let request = CompletionRequest::new("gpt-4")
            .with_message(Message::system("You are helpful"))
            .with_message(Message::user("Hello"))
            .with_max_tokens(100)
            .with_temperature(0.7);

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.max_tokens, Some(100));
        assert_eq!(request.temperature, Some(0.7));
    }

    #[test]
    fn test_tool_completion_request() {
        let request = CompletionRequest::new("gpt-4");
        let tools = vec![ToolDefinition::new(
            "test",
            "Test tool",
            serde_json::json!({}),
        )];

        let tool_request =
            ToolCompletionRequest::new(request, tools).with_tool_choice(ToolChoice::Required);

        assert!(matches!(tool_request.tool_choice, ToolChoice::Required));
        assert_eq!(tool_request.tools.len(), 1);
    }

    #[test]
    fn test_tool_completion_response_has_tool_calls() {
        let response = ToolCompletionResponse {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_1".to_string(),
                name: "test".to_string(),
                arguments: "{}".to_string(),
            }],
            usage: None,
            finish_reason: Some("tool_calls".to_string()),
            model: "gpt-4".to_string(),
        };

        assert!(response.has_tool_calls());

        let empty_response = ToolCompletionResponse {
            content: Some("Hello".to_string()),
            tool_calls: vec![],
            usage: None,
            finish_reason: Some("stop".to_string()),
            model: "gpt-4".to_string(),
        };

        assert!(!empty_response.has_tool_calls());
    }
}
