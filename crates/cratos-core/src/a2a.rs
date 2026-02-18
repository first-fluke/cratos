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
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_and_receive() {
        let router = A2aRouter::new(100);

        let msg = A2aMessage::new("backend", "frontend", "session-1", "API ready");
        router.send(msg).await;

        // Recipient should see the message
        let msgs = router.receive("frontend").await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].from_agent, "backend");
        assert_eq!(msgs[0].content, "API ready");

        // Queue should be drained
        let msgs = router.receive("frontend").await;
        assert!(msgs.is_empty());
    }

    #[tokio::test]
    async fn test_peek_does_not_drain() {
        let router = A2aRouter::new(100);

        let msg = A2aMessage::new("backend", "frontend", "s1", "hello");
        router.send(msg).await;

        let peeked = router.peek("frontend").await;
        assert_eq!(peeked.len(), 1);

        // Peek again — still there
        let peeked = router.peek("frontend").await;
        assert_eq!(peeked.len(), 1);

        // Receive drains
        let received = router.receive("frontend").await;
        assert_eq!(received.len(), 1);
        assert!(router.peek("frontend").await.is_empty());
    }

    #[tokio::test]
    async fn test_session_history() {
        let router = A2aRouter::new(100);

        router
            .send(A2aMessage::new("backend", "frontend", "s1", "step 1"))
            .await;
        router
            .send(A2aMessage::new("frontend", "qa", "s1", "step 2"))
            .await;
        router
            .send(A2aMessage::new(
                "backend",
                "frontend",
                "s2",
                "other session",
            ))
            .await;

        let history = router.session_history("s1").await;
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].content, "step 1");
        assert_eq!(history[1].content, "step 2");

        // Other session
        let history2 = router.session_history("s2").await;
        assert_eq!(history2.len(), 1);
    }

    #[tokio::test]
    async fn test_history_limit() {
        let router = A2aRouter::new(3);

        for i in 0..5 {
            router
                .send(A2aMessage::new("a", "b", "s1", format!("msg {}", i)))
                .await;
        }

        let history = router.session_history("s1").await;
        assert_eq!(history.len(), 3);
        // Oldest messages should be dropped
        assert_eq!(history[0].content, "msg 2");
        assert_eq!(history[2].content, "msg 4");
    }

    #[tokio::test]
    async fn test_pending_count() {
        let router = A2aRouter::new(100);

        assert_eq!(router.pending_count("frontend").await, 0);

        router
            .send(A2aMessage::new("a", "frontend", "s1", "1"))
            .await;
        router
            .send(A2aMessage::new("b", "frontend", "s1", "2"))
            .await;

        assert_eq!(router.pending_count("frontend").await, 2);

        router.receive("frontend").await;
        assert_eq!(router.pending_count("frontend").await, 0);
    }

    #[tokio::test]
    async fn test_clear_session() {
        let router = A2aRouter::new(100);

        router.send(A2aMessage::new("a", "b", "s1", "msg1")).await;
        router.send(A2aMessage::new("a", "b", "s2", "msg2")).await;

        router.clear_session("s1").await;

        // s1 history cleared
        assert!(router.session_history("s1").await.is_empty());
        // s2 unaffected
        assert_eq!(router.session_history("s2").await.len(), 1);

        // s1 messages removed from queue (b had 2 messages, now only s2's)
        let msgs = router.receive("b").await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].session_id, "s2");
    }

    #[tokio::test]
    async fn test_message_summary() {
        let router = A2aRouter::new(100);

        let long_content = "x".repeat(300);
        router
            .send(A2aMessage::new("a", "b", "s1", long_content))
            .await;

        let summaries = router.session_history_summaries("s1").await;
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].preview.len() < 210); // 200 + "..."
        assert!(summaries[0].preview.ends_with("..."));
    }

    #[tokio::test]
    async fn test_message_with_metadata() {
        let msg = A2aMessage::new("a", "b", "s1", "hello")
            .with_metadata("priority", serde_json::json!("high"))
            .with_metadata("tags", serde_json::json!(["urgent"]));

        assert_eq!(msg.metadata["priority"], "high");
        assert_eq!(msg.metadata.len(), 2);
    }

    #[tokio::test]
    async fn test_multiple_agents_isolation() {
        let router = A2aRouter::new(100);

        router
            .send(A2aMessage::new("a", "frontend", "s1", "for frontend"))
            .await;
        router
            .send(A2aMessage::new("a", "backend", "s1", "for backend"))
            .await;

        let fe_msgs = router.receive("frontend").await;
        assert_eq!(fe_msgs.len(), 1);
        assert_eq!(fe_msgs[0].content, "for frontend");

        let be_msgs = router.receive("backend").await;
        assert_eq!(be_msgs.len(), 1);
        assert_eq!(be_msgs[0].content, "for backend");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let msg = A2aMessage::new("backend", "frontend", "s1", "test content");
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: A2aMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.from_agent, "backend");
        assert_eq!(parsed.to_agent, "frontend");
        assert_eq!(parsed.content, "test content");
    }
}
