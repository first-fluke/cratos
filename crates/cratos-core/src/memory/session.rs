//! Session context management
//!
//! This module provides token-aware session context management for LLM conversations.
//!
//! ## Token Budget Management
//!
//! Instead of limiting by message count (which treats "hello" the same as a 5000-token
//! code review), this module uses actual token counting for intelligent context trimming.
//!
//! - Default token budget: 100,000 tokens per session
//! - Importance-weighted trimming: Preserves tool_result > user > assistant messages
//! - System messages are always preserved

use chrono::{DateTime, Utc};
use cratos_llm::{count_message_tokens, Message, MessageRole};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};
use uuid::Uuid;

/// Maximum number of messages to keep in session context (legacy fallback)
const DEFAULT_MAX_CONTEXT_SIZE: usize = 50;

/// Default maximum tokens per session context
const DEFAULT_MAX_TOKENS: usize = 100_000;

/// Message importance levels for trimming priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MessageImportance {
    /// System messages - never trimmed
    System = 4,
    /// Tool results - high priority (expensive to regenerate)
    ToolResult = 3,
    /// User messages - medium-high priority
    User = 2,
    /// Assistant messages - lower priority (can be regenerated)
    Assistant = 1,
}

impl MessageImportance {
    fn from_role(role: MessageRole) -> Self {
        match role {
            MessageRole::System => Self::System,
            MessageRole::Tool => Self::ToolResult,
            MessageRole::User => Self::User,
            MessageRole::Assistant => Self::Assistant,
        }
    }
}

/// Session context for a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    /// Session ID
    pub id: Uuid,
    /// Session key (channel_type:channel_id:user_id)
    pub session_key: String,
    /// Conversation messages
    pub messages: Vec<Message>,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    /// Session metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Maximum context size (message count - legacy)
    #[serde(default = "default_max_context_size")]
    pub max_context_size: usize,
    /// Maximum tokens for the session context
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    /// Current token count (cached for efficiency)
    #[serde(default)]
    current_tokens: usize,
    /// Enable token-aware trimming (vs legacy message-count)
    #[serde(default = "default_token_aware")]
    pub token_aware_trimming: bool,
}

fn default_max_context_size() -> usize {
    DEFAULT_MAX_CONTEXT_SIZE
}

fn default_max_tokens() -> usize {
    DEFAULT_MAX_TOKENS
}

fn default_token_aware() -> bool {
    true
}

impl SessionContext {
    /// Create a new session context with token-aware trimming
    #[must_use]
    pub fn new(session_key: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_key: session_key.into(),
            messages: Vec::new(),
            last_activity: Utc::now(),
            metadata: HashMap::new(),
            max_context_size: DEFAULT_MAX_CONTEXT_SIZE,
            max_tokens: DEFAULT_MAX_TOKENS,
            current_tokens: 0,
            token_aware_trimming: true,
        }
    }

    /// Create a session context with custom token budget
    #[must_use]
    pub fn with_token_budget(session_key: impl Into<String>, max_tokens: usize) -> Self {
        Self {
            max_tokens,
            ..Self::new(session_key)
        }
    }

    /// Create a legacy session context (message-count based trimming)
    #[must_use]
    pub fn legacy(session_key: impl Into<String>) -> Self {
        Self {
            token_aware_trimming: false,
            ..Self::new(session_key)
        }
    }

    /// Create a session key from channel info
    #[must_use]
    pub fn make_key(channel_type: &str, channel_id: &str, user_id: &str) -> String {
        format!("{}:{}:{}", channel_type, channel_id, user_id)
    }

    /// Add a user message
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::user(content));
        self.last_activity = Utc::now();
        self.trim_if_needed();
    }

    /// Add an assistant message
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::assistant(content));
        self.last_activity = Utc::now();
        self.trim_if_needed();
    }

    /// Add a system message
    pub fn add_system_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message::system(content));
        self.last_activity = Utc::now();
        self.trim_if_needed();
    }

    /// Add a tool message
    pub fn add_tool_message(
        &mut self,
        content: impl Into<String>,
        tool_call_id: impl Into<String>,
    ) {
        self.messages
            .push(Message::tool_response(tool_call_id, content));
        self.last_activity = Utc::now();
        self.trim_if_needed();
    }

    /// Get messages for LLM context
    #[must_use]
    pub fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get messages with a system prompt prepended
    #[must_use]
    pub fn get_messages_with_system(&self, system_prompt: &str) -> Vec<Message> {
        let mut messages = vec![Message::system(system_prompt)];
        messages.extend(self.messages.clone());
        messages
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
        self.last_activity = Utc::now();
    }

    /// Set metadata value
    pub fn set_metadata(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.metadata.insert(key.into(), value);
    }

    /// Get metadata value
    #[must_use]
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }

    /// Trim messages if exceeding limits
    ///
    /// Uses token-aware trimming by default, which:
    /// 1. Preserves system messages (never trimmed)
    /// 2. Prioritizes by importance: tool_result > user > assistant
    /// 3. Removes oldest low-priority messages first
    fn trim_if_needed(&mut self) {
        if self.token_aware_trimming {
            self.trim_by_tokens();
        } else {
            self.trim_by_count();
        }
    }

    /// Legacy message-count based trimming
    fn trim_by_count(&mut self) {
        if self.messages.len() > self.max_context_size {
            let excess = self.messages.len() - self.max_context_size;
            self.messages.drain(0..excess);
        }
    }

    /// Token-aware trimming with importance weighting
    fn trim_by_tokens(&mut self) {
        // Recalculate current token count
        self.current_tokens = count_message_tokens(&self.messages);

        if self.current_tokens <= self.max_tokens {
            return;
        }

        debug!(
            session_id = %self.id,
            current_tokens = self.current_tokens,
            max_tokens = self.max_tokens,
            "Trimming session context by tokens"
        );

        // Separate system messages (always kept) from others
        let mut system_messages = Vec::new();
        let mut other_messages: Vec<(usize, Message)> = Vec::new();

        for (idx, msg) in self.messages.drain(..).enumerate() {
            if msg.role == MessageRole::System {
                system_messages.push(msg);
            } else {
                other_messages.push((idx, msg));
            }
        }

        // Sort non-system messages by importance (ascending) then by age (oldest first)
        other_messages.sort_by(|(idx_a, msg_a), (idx_b, msg_b)| {
            let imp_a = MessageImportance::from_role(msg_a.role);
            let imp_b = MessageImportance::from_role(msg_b.role);

            // Lower importance = trim first, older = trim first
            imp_a.cmp(&imp_b).then_with(|| idx_a.cmp(idx_b))
        });

        // Calculate system message tokens (always kept)
        let system_tokens = count_message_tokens(&system_messages);
        let available_tokens = self.max_tokens.saturating_sub(system_tokens);

        // Keep messages until we hit the token budget
        let mut kept_messages: Vec<(usize, Message)> = Vec::new();
        let mut kept_tokens = 0usize;

        // Iterate from highest importance (end) to lowest (start)
        for (idx, msg) in other_messages.into_iter().rev() {
            let msg_tokens = count_message_tokens(std::slice::from_ref(&msg));

            if kept_tokens + msg_tokens <= available_tokens {
                kept_tokens += msg_tokens;
                kept_messages.push((idx, msg));
            } else {
                debug!(
                    role = ?msg.role,
                    tokens = msg_tokens,
                    "Trimming message to fit token budget"
                );
            }
        }

        // Restore original order
        kept_messages.sort_by_key(|(idx, _)| *idx);

        // Rebuild messages: system first, then others in order
        self.messages = system_messages;
        self.messages
            .extend(kept_messages.into_iter().map(|(_, msg)| msg));

        // Update cached token count
        self.current_tokens = count_message_tokens(&self.messages);

        warn!(
            session_id = %self.id,
            new_token_count = self.current_tokens,
            messages_kept = self.messages.len(),
            "Session context trimmed"
        );
    }

    /// Get current token count
    #[must_use]
    pub fn token_count(&self) -> usize {
        if self.current_tokens == 0 && !self.messages.is_empty() {
            count_message_tokens(&self.messages)
        } else {
            self.current_tokens
        }
    }

    /// Get remaining token budget
    #[must_use]
    pub fn remaining_tokens(&self) -> usize {
        self.max_tokens.saturating_sub(self.token_count())
    }

    /// Check if adding a message would exceed token budget
    #[must_use]
    pub fn would_exceed_budget(&self, message: &Message) -> bool {
        let msg_tokens = count_message_tokens(std::slice::from_ref(message));
        self.token_count() + msg_tokens > self.max_tokens
    }

    /// Get message count
    #[must_use]
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_context() {
        let mut ctx = SessionContext::new("test:channel:user");
        assert_eq!(ctx.message_count(), 0);

        ctx.add_user_message("Hello");
        ctx.add_assistant_message("Hi there!");
        assert_eq!(ctx.message_count(), 2);

        let messages = ctx.get_messages();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_session_key() {
        let key = SessionContext::make_key("telegram", "123", "456");
        assert_eq!(key, "telegram:123:456");
    }

    #[test]
    fn test_trim_messages_legacy() {
        let mut ctx = SessionContext::legacy("test:key");
        ctx.max_context_size = 3;

        for i in 0..5 {
            ctx.add_user_message(format!("Message {}", i));
        }

        assert_eq!(ctx.message_count(), 3);
    }

    #[test]
    fn test_metadata() {
        let mut ctx = SessionContext::new("test:key");
        ctx.set_metadata("key1", serde_json::json!("value1"));

        assert_eq!(ctx.get_metadata("key1"), Some(&serde_json::json!("value1")));
        assert_eq!(ctx.get_metadata("nonexistent"), None);
    }

    // ========================================================================
    // Token-Aware Trimming Tests
    // ========================================================================

    #[test]
    fn test_token_count() {
        let mut ctx = SessionContext::new("test:key");
        assert_eq!(ctx.token_count(), 0);

        ctx.add_user_message("Hello, world!");
        assert!(ctx.token_count() > 0);
    }

    #[test]
    fn test_with_token_budget() {
        let ctx = SessionContext::with_token_budget("test:key", 50_000);
        assert_eq!(ctx.max_tokens, 50_000);
        assert!(ctx.token_aware_trimming);
    }

    #[test]
    fn test_remaining_tokens() {
        let mut ctx = SessionContext::with_token_budget("test:key", 1000);

        let initial = ctx.remaining_tokens();
        assert_eq!(initial, 1000);

        ctx.add_user_message("Hello!");
        assert!(ctx.remaining_tokens() < initial);
    }

    #[test]
    fn test_would_exceed_budget() {
        let ctx = SessionContext::with_token_budget("test:key", 100);

        // Small message should not exceed
        let small_msg = Message::user("Hi");
        assert!(!ctx.would_exceed_budget(&small_msg));

        // Large message should exceed
        let large_msg = Message::user("A".repeat(1000));
        assert!(ctx.would_exceed_budget(&large_msg));
    }

    #[test]
    fn test_token_aware_trimming_preserves_system() {
        let mut ctx = SessionContext::with_token_budget("test:key", 500);

        ctx.add_system_message("You are a helpful assistant.");
        ctx.add_user_message("Hello!");
        ctx.add_assistant_message("Hi there! How can I help?");

        // Add messages until we exceed budget
        for i in 0..20 {
            ctx.add_user_message(format!("Question {}: What is the meaning of life?", i));
            ctx.add_assistant_message(format!("Answer {}: 42", i));
        }

        // System message should be preserved
        let messages = ctx.get_messages();
        assert!(messages.iter().any(|m| m.role == MessageRole::System));
    }

    #[test]
    fn test_token_aware_trimming_prioritizes_importance() {
        let mut ctx = SessionContext::with_token_budget("test:key", 300);

        // Add messages of different types
        ctx.add_user_message("User message 1");
        ctx.add_assistant_message("Assistant response 1 - this is lower priority");
        ctx.add_user_message("User message 2");
        ctx.add_tool_message("Tool result - high priority", "tool_1");
        ctx.add_assistant_message("Assistant response 2 - this is lower priority");

        // Force trimming by adding more
        for _ in 0..10 {
            ctx.add_assistant_message("More assistant text to force trimming");
        }

        // Tool results should be more likely to survive than assistant messages
        let messages = ctx.get_messages();
        let has_tool = messages.iter().any(|m| m.role == MessageRole::Tool);
        let assistant_count = messages
            .iter()
            .filter(|m| m.role == MessageRole::Assistant)
            .count();

        // If there's limited space, tool messages should be preserved over assistant
        if messages.len() < 10 {
            // With tight budget, tool results should survive
            assert!(has_tool || assistant_count == 0);
        }
    }

    #[test]
    fn test_message_importance_ordering() {
        assert!(MessageImportance::System > MessageImportance::ToolResult);
        assert!(MessageImportance::ToolResult > MessageImportance::User);
        assert!(MessageImportance::User > MessageImportance::Assistant);
    }
}
