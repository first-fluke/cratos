//! Session Send Tool - Send messages to other agents
//!
//! Allows agents to send A2A (Agent-to-Agent) messages within a session.
//! Requires a `MessageSender` implementation (injected from core).

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;

/// Trait for sending A2A messages (implemented by core::A2aRouter)
#[async_trait]
pub trait MessageSender: Send + Sync + std::fmt::Debug {
    /// Send a message to another agent
    async fn send(
        &self,
        from_agent: &str,
        to_agent: &str,
        content: &str,
        session_id: &str,
    ) -> anyhow::Result<()>;
}

#[derive(Deserialize)]
struct SessionSendInput {
    to_agent: String,
    content: String,
    session_id: String,
}

/// Tool for sending messages to other agents
pub struct SessionSendTool {
    definition: ToolDefinition,
    sender: Arc<dyn MessageSender>,
    current_agent: String,
}

impl SessionSendTool {
    /// Create a new session send tool
    pub fn new(sender: Arc<dyn MessageSender>, current_agent: impl Into<String>) -> Self {
        let definition = ToolDefinition::new(
            "session_send",
            "Send a message to another agent in the same session. \
             Use this to coordinate with other agents (e.g., frontend, mobile, qa).",
        )
        .with_category(ToolCategory::Utility)
        .with_risk_level(RiskLevel::Low)
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "to_agent": {
                    "type": "string",
                    "description": "Target agent ID (e.g., 'frontend', 'mobile')"
                },
                "content": {
                    "type": "string",
                    "description": "Message content to send"
                },
                "session_id": {
                    "type": "string",
                    "description": "Current session ID"
                }
            },
            "required": ["to_agent", "content", "session_id"]
        }));

        Self {
            definition,
            sender,
            current_agent: current_agent.into(),
        }
    }
}

#[async_trait]
impl Tool for SessionSendTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let params: SessionSendInput = serde_json::from_value(input)
            .map_err(|e| Error::InvalidInput(format!("Invalid arguments: {}", e)))?;

        let start = std::time::Instant::now();

        self.sender
            .send(
                &self.current_agent,
                &params.to_agent,
                &params.content,
                &params.session_id,
            )
            .await
            .map_err(|e| Error::Execution(format!("Failed to send message: {}", e)))?;

        Ok(ToolResult::success(
            serde_json::json!({
                "status": "sent",
                "to": params.to_agent,
                "session": params.session_id
            }),
            start.elapsed().as_millis() as u64,
        ))
    }
}
