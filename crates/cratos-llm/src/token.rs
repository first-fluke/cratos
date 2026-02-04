//! Token counting and budget management
//!
//! This module provides token counting utilities using tiktoken's cl100k_base encoding,
//! which is compatible with most modern LLMs.

use crate::message::Message;
use crate::tools::ToolDefinition;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tiktoken_rs::{cl100k_base, CoreBPE};

/// Global tokenizer instance (initialized once, thread-safe)
static TOKENIZER: LazyLock<CoreBPE> = LazyLock::new(|| {
    cl100k_base().expect("cl100k_base tokenizer is a compile-time constant and should never fail")
});

// ============================================================================
// Token Counter
// ============================================================================

/// Token counter for estimating message token usage
///
/// Uses tiktoken's cl100k_base encoding (GPT-5.2, Claude 3, etc.)
/// for accurate token estimation across modern LLMs.
///
/// This is a zero-cost wrapper around the global tokenizer instance.
#[derive(Clone, Copy)]
pub struct TokenCounter;

impl TokenCounter {
    /// Create a new token counter
    ///
    /// Uses cl100k_base encoding which is compatible with:
    /// - OpenAI GPT-4, GPT-5.2, GPT-3.5-turbo
    /// - Anthropic Claude 3.x (approximate)
    /// - Most modern LLMs
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Count tokens in a string
    #[must_use]
    pub fn count_tokens(&self, text: &str) -> usize {
        TOKENIZER.encode_with_special_tokens(text).len()
    }

    /// Count tokens in a message (includes role overhead)
    ///
    /// Adds overhead for message structure:
    /// - Role marker: ~4 tokens
    /// - Message separators: ~2 tokens
    #[must_use]
    pub fn count_message_tokens(&self, message: &Message) -> usize {
        const MESSAGE_OVERHEAD: usize = 6; // role + separators
        self.count_tokens(&message.content) + MESSAGE_OVERHEAD
    }

    /// Count total tokens in a conversation
    #[must_use]
    pub fn count_conversation_tokens(&self, messages: &[Message]) -> usize {
        const CONVERSATION_OVERHEAD: usize = 3; // start/end tokens
        messages
            .iter()
            .map(|m| self.count_message_tokens(m))
            .sum::<usize>()
            + CONVERSATION_OVERHEAD
    }

    /// Estimate tokens for a tool definition
    #[must_use]
    pub fn count_tool_tokens(&self, tool: &ToolDefinition) -> usize {
        const TOOL_OVERHEAD: usize = 10; // structure overhead
        self.count_tokens(&tool.name)
            + self.count_tokens(&tool.description)
            + self.count_tokens(&tool.parameters.to_string())
            + TOOL_OVERHEAD
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

// Thread-safe global token counter
lazy_static::lazy_static! {
    /// Global token counter instance for convenience
    pub static ref TOKEN_COUNTER: TokenCounter = TokenCounter::new();
}

/// Convenience function to count tokens in text
#[must_use]
pub fn count_tokens(text: &str) -> usize {
    TOKEN_COUNTER.count_tokens(text)
}

/// Convenience function to count tokens in messages
#[must_use]
pub fn count_message_tokens(messages: &[Message]) -> usize {
    TOKEN_COUNTER.count_conversation_tokens(messages)
}

// ============================================================================
// Token Budget
// ============================================================================

/// Token budget configuration for different task types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Maximum tokens to generate for this task type
    pub max_tokens: u32,
    /// Recommended temperature for this task type
    pub temperature: f32,
}

impl TokenBudget {
    /// Create a new token budget
    #[must_use]
    pub const fn new(max_tokens: u32, temperature: f32) -> Self {
        Self {
            max_tokens,
            temperature,
        }
    }
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_counter_basic() {
        let counter = TokenCounter::new();

        // Simple text
        let tokens = counter.count_tokens("Hello, world!");
        assert!(tokens > 0);
        assert!(tokens < 10); // Should be ~4 tokens

        // Empty string
        assert_eq!(counter.count_tokens(""), 0);
    }

    #[test]
    fn test_token_counter_message() {
        let counter = TokenCounter::new();

        let message = Message::user("Hello, how are you?");
        let tokens = counter.count_message_tokens(&message);

        // Should include content + overhead
        let content_tokens = counter.count_tokens("Hello, how are you?");
        assert!(tokens > content_tokens);
    }

    #[test]
    fn test_token_counter_conversation() {
        let counter = TokenCounter::new();

        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello!"),
            Message::assistant("Hi there! How can I help you?"),
        ];

        let total = counter.count_conversation_tokens(&messages);

        // Should be sum of messages + overhead
        let sum: usize = messages
            .iter()
            .map(|m| counter.count_message_tokens(m))
            .sum();
        assert!(total >= sum);
    }

    #[test]
    fn test_token_counter_tool() {
        let counter = TokenCounter::new();

        let tool = ToolDefinition::new(
            "get_weather",
            "Get the current weather for a location",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string", "description": "City name"}
                },
                "required": ["location"]
            }),
        );

        let tokens = counter.count_tool_tokens(&tool);
        assert!(tokens > 0);
    }

    #[test]
    fn test_global_token_counter() {
        // Test convenience functions
        let tokens = count_tokens("Hello, world!");
        assert!(tokens > 0);

        let messages = vec![Message::user("Hello!")];
        let msg_tokens = count_message_tokens(&messages);
        assert!(msg_tokens > tokens); // Should include overhead
    }

    #[test]
    fn test_token_budget_default() {
        let budget = TokenBudget::default();
        assert_eq!(budget.max_tokens, 2048);
        assert_eq!(budget.temperature, 0.7);
    }

    #[test]
    fn test_token_budget_new() {
        let budget = TokenBudget::new(500, 0.3);
        assert_eq!(budget.max_tokens, 500);
        assert_eq!(budget.temperature, 0.3);
    }
}
