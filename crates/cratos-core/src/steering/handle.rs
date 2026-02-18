use super::types::{SteerError, SteerMessage};
use chrono::Utc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Handle to control the agent execution from external components (e.g., UI, Queue).
#[derive(Clone)]
pub struct SteerHandle {
    steer_tx: mpsc::Sender<SteerMessage>,
    execution_id: Uuid,
}

impl SteerHandle {
    /// Create a new steering handle.
    pub fn new(steer_tx: mpsc::Sender<SteerMessage>, execution_id: Uuid) -> Self {
        Self {
            steer_tx,
            execution_id,
        }
    }

    /// Inject a user message during execution.
    pub async fn inject_message(&self, content: String) -> Result<(), SteerError> {
        self.steer_tx
            .send(SteerMessage::UserText {
                content,
                timestamp: Utc::now(),
            })
            .await
            .map_err(|_| SteerError::ChannelClosed)
    }

    /// Abort the execution immediately.
    pub async fn abort(&self, reason: Option<String>) -> Result<(), SteerError> {
        self.steer_tx
            .send(SteerMessage::Abort { reason })
            .await
            .map_err(|_| SteerError::ChannelClosed)
    }

    /// Skip a specific tool execution.
    pub async fn skip_tool(&self, tool_call_id: String) -> Result<(), SteerError> {
        self.steer_tx
            .send(SteerMessage::SkipTool { tool_call_id })
            .await
            .map_err(|_| SteerError::ChannelClosed)
    }

    /// Get the execution ID associated with this handle.
    #[must_use]
    pub fn execution_id(&self) -> Uuid {
        self.execution_id
    }
}
