//! Session context management

use chrono::{DateTime, Utc};
use cratos_llm::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Maximum number of messages to keep in session context
const DEFAULT_MAX_CONTEXT_SIZE: usize = 50;

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
    /// Maximum context size
    #[serde(default = "default_max_context_size")]
    pub max_context_size: usize,
}

fn default_max_context_size() -> usize {
    DEFAULT_MAX_CONTEXT_SIZE
}

impl SessionContext {
    /// Create a new session context
    #[must_use]
    pub fn new(session_key: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_key: session_key.into(),
            messages: Vec::new(),
            last_activity: Utc::now(),
            metadata: HashMap::new(),
            max_context_size: DEFAULT_MAX_CONTEXT_SIZE,
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

    /// Trim messages if exceeding max size
    fn trim_if_needed(&mut self) {
        if self.messages.len() > self.max_context_size {
            let excess = self.messages.len() - self.max_context_size;
            self.messages.drain(0..excess);
        }
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
    fn test_trim_messages() {
        let mut ctx = SessionContext::new("test:key");
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
}
