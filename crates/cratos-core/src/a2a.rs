//! A2A (Agent-to-Agent) message passing.
//!
//! Enables agents to communicate directly with each other during multi-agent
//! orchestration. When `@backend API @frontend UI` is processed, the backend
//! agent's result is forwarded to the frontend agent as A2A context.
//!
//! ## Design
//!
//! - `A2aMessage`: A message from one agent to another within a session
//! - `A2aRouter`: In-memory message queue + history per agent/session
//! - EventBus integration: A2A events are published for WS/ACP forwarding

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use tokio::sync::RwLock;
use uuid::Uuid;

/// A message from one agent to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aMessage {
    /// Unique message ID
    pub id: Uuid,
    /// Sending agent ID (e.g., "backend")
    pub from_agent: String,
    /// Receiving agent ID (e.g., "frontend")
    pub to_agent: String,
    /// Session context
    pub session_id: String,
    /// Message content (typically the agent's response)
    pub content: String,
    /// When the message was created
    pub created_at: DateTime<Utc>,
    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl A2aMessage {
    /// Create a new A2A message.
    pub fn new(
        from_agent: impl Into<String>,
        to_agent: impl Into<String>,
        session_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_agent: from_agent.into(),
            to_agent: to_agent.into(),
            session_id: session_id.into(),
            content: content.into(),
            created_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the message.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Summary of an A2A message (for list endpoints).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aMessageSummary {
    /// Message ID
    pub id: Uuid,
    /// Sending agent
    pub from_agent: String,
    /// Receiving agent
    pub to_agent: String,
    /// Session ID
    pub session_id: String,
    /// Truncated content preview (first 200 chars)
    pub preview: String,
    /// When created
    pub created_at: DateTime<Utc>,
}

impl From<&A2aMessage> for A2aMessageSummary {
    fn from(msg: &A2aMessage) -> Self {
        let preview = if msg.content.len() <= 200 {
            msg.content.clone()
        } else {
            // Safe UTF-8 truncation
            let end = msg
                .content
                .char_indices()
                .take_while(|(i, _)| *i < 200)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);
            format!("{}...", &msg.content[..end])
        };
        Self {
            id: msg.id,
            from_agent: msg.from_agent.clone(),
            to_agent: msg.to_agent.clone(),
            session_id: msg.session_id.clone(),
            preview,
            created_at: msg.created_at,
        }
    }
}

/// A2A message router.
///
/// Maintains per-agent message queues and per-session history.
/// Thread-safe via `RwLock`.
#[derive(Debug)]
pub struct A2aRouter {
    /// Pending messages per agent: agent_id → queue
    queues: RwLock<HashMap<String, VecDeque<A2aMessage>>>,
    /// Session history: session_id → messages (ordered by time)
    history: RwLock<HashMap<String, Vec<A2aMessage>>>,
    /// Max messages per session history (prevents unbounded growth)
    max_history: usize,
}

impl A2aRouter {
    /// Create a new A2A router.
    pub fn new(max_history: usize) -> Self {
        Self {
            queues: RwLock::new(HashMap::new()),
            history: RwLock::new(HashMap::new()),
            max_history,
        }
    }

    /// Send a message to an agent.
    ///
    /// The message is added to the recipient's queue and to the session history.
    pub async fn send(&self, msg: A2aMessage) {
        // Add to recipient's queue
        {
            let mut queues = self.queues.write().await;
            queues
                .entry(msg.to_agent.clone())
                .or_default()
                .push_back(msg.clone());
        }

        // Add to session history
        {
            let mut history = self.history.write().await;
            let entries = history.entry(msg.session_id.clone()).or_default();
            entries.push(msg);

            // Trim if over limit
            if entries.len() > self.max_history {
                let excess = entries.len() - self.max_history;
                entries.drain(..excess);
            }
        }
    }

    /// Receive all pending messages for an agent (drains the queue).
    pub async fn receive(&self, agent_id: &str) -> Vec<A2aMessage> {
        let mut queues = self.queues.write().await;
        queues.remove(agent_id).map(Vec::from).unwrap_or_default()
    }

    /// Peek at pending messages for an agent without draining.
    pub async fn peek(&self, agent_id: &str) -> Vec<A2aMessage> {
        let queues = self.queues.read().await;
        queues
            .get(agent_id)
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get session history (all messages in a session).
    pub async fn session_history(&self, session_id: &str) -> Vec<A2aMessage> {
        let history = self.history.read().await;
        history.get(session_id).cloned().unwrap_or_default()
    }

    /// Get session history as summaries.
    pub async fn session_history_summaries(&self, session_id: &str) -> Vec<A2aMessageSummary> {
        let history = self.history.read().await;
        history
            .get(session_id)
            .map(|msgs| msgs.iter().map(A2aMessageSummary::from).collect())
            .unwrap_or_default()
    }

    /// Get pending message count for an agent.
    pub async fn pending_count(&self, agent_id: &str) -> usize {
        let queues = self.queues.read().await;
        queues.get(agent_id).map(|q| q.len()).unwrap_or(0)
    }

    /// Clear all data for a session (cleanup).
    pub async fn clear_session(&self, session_id: &str) {
        // Remove from history
        {
            let mut history = self.history.write().await;
            history.remove(session_id);
        }

        // Remove queued messages for this session
        {
            let mut queues = self.queues.write().await;
            for queue in queues.values_mut() {
                queue.retain(|msg| msg.session_id != session_id);
            }
        }
    }
}

#[async_trait::async_trait]
impl cratos_tools::builtins::MessageSender for A2aRouter {
    async fn send(
        &self,
        from_agent: &str,
        to_agent: &str,
        content: &str,
        session_id: &str,
    ) -> anyhow::Result<()> {
        let msg = A2aMessage::new(from_agent, to_agent, session_id, content);
        self.send(msg).await;
        Ok(())
    }
}

impl Default for A2aRouter {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[cfg(test)]
mod tests;

