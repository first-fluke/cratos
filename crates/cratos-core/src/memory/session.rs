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
    ///
    /// Only the FIRST system message (original system prompt) is sacred and never trimmed.
    /// Supplementary system messages (memory injections, RAG context) are trimmable
    /// at the same priority as user messages.
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

        // Only the FIRST system message is sacred (never trimmed).
        // All other messages (including supplementary system messages) are trimmable.
        let mut sacred_system: Option<Message> = None;
        let mut trimmable: Vec<(usize, Message)> = Vec::new();
        let mut seen_first_system = false;

        for (idx, msg) in self.messages.drain(..).enumerate() {
            if msg.role == MessageRole::System && !seen_first_system {
                sacred_system = Some(msg);
                seen_first_system = true;
            } else {
                trimmable.push((idx, msg));
            }
        }

        // Sort by importance (ascending) then by age (oldest first).
        // Supplementary system messages get User-level importance (trimmable).
        trimmable.sort_by(|(idx_a, msg_a), (idx_b, msg_b)| {
            let imp_a = if msg_a.role == MessageRole::System {
                MessageImportance::User // supplementary system = same priority as user
            } else {
                MessageImportance::from_role(msg_a.role)
            };
            let imp_b = if msg_b.role == MessageRole::System {
                MessageImportance::User
            } else {
                MessageImportance::from_role(msg_b.role)
            };

            imp_a.cmp(&imp_b).then_with(|| idx_a.cmp(idx_b))
        });

        // Calculate sacred tokens (first system message only)
        let sacred_tokens = sacred_system
            .as_ref()
            .map(|m| count_message_tokens(std::slice::from_ref(m)))
            .unwrap_or(0);
        let available_tokens = self.max_tokens.saturating_sub(sacred_tokens);

        // Keep messages until we hit the token budget
        let mut kept_messages: Vec<(usize, Message)> = Vec::new();
        let mut kept_tokens = 0usize;

        // Iterate from highest importance (end) to lowest (start)
        for (idx, msg) in trimmable.into_iter().rev() {
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

        // Rebuild messages: sacred system first, then others in order
        self.messages = Vec::new();
        if let Some(sys) = sacred_system {
            self.messages.push(sys);
        }
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

    /// Replace the middle of the conversation with Graph RAG retrieved turns.
    ///
    /// Preserves system messages (beginning) and the current turn (end),
    /// replacing everything in between with the most relevant past turns.
    pub fn replace_with_retrieved(&mut self, retrieved_messages: Vec<Message>) {
        // Separate system messages
        let system_msgs: Vec<Message> = self
            .messages
            .iter()
            .filter(|m| m.role == MessageRole::System)
            .cloned()
            .collect();

        // Keep the last user message (current turn)
        let current_turn = self
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .cloned();

        // Rebuild: system + retrieved context + current turn
        self.messages = system_msgs;
        self.messages.extend(retrieved_messages);
        if let Some(turn) = current_turn {
            self.messages.push(turn);
        }

        // Update cached token count
        self.current_tokens = count_message_tokens(&self.messages);

        debug!(
            session_id = %self.id,
            new_token_count = self.current_tokens,
            messages = self.messages.len(),
            "Session context replaced with Graph RAG results"
        );
    }

    /// Insert supplementary context messages after system messages.
    ///
    /// Used by Graph RAG to add relevant past turns at the beginning of
    /// a conversation without replacing existing messages.
    /// Triggers trimming after insertion to prevent unbounded growth.
    pub fn insert_supplementary_context(&mut self, context_messages: Vec<Message>) {
        if context_messages.is_empty() {
            return;
        }
        let insert_pos = self
            .messages
            .iter()
            .take_while(|m| m.role == MessageRole::System)
            .count();
        for (i, msg) in context_messages.into_iter().enumerate() {
            self.messages.insert(insert_pos + i, msg);
        }
        self.current_tokens = count_message_tokens(&self.messages);
        self.trim_if_needed();
    }

    /// Remove system messages whose content starts with the given prefix.
    ///
    /// Used to deduplicate memory injections: before adding new memory
    /// system messages, remove old ones with the same marker prefix.
    pub fn remove_system_messages_with_prefix(&mut self, prefix: &str) {
        let before = self.messages.len();
        self.messages.retain(|m| {
            !(m.role == MessageRole::System && m.content.starts_with(prefix))
        });
        let removed = before - self.messages.len();
        if removed > 0 {
            self.current_tokens = count_message_tokens(&self.messages);
            debug!(
                session_id = %self.id,
                removed = removed,
                prefix = %prefix,
                "Removed duplicate supplementary system messages"
            );
        }
    }

    /// Get message count
    #[must_use]
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

#[cfg(test)]
mod tests;

