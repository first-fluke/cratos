use super::handle::SteerHandle;
use super::types::{SteerDecision, SteerError, SteerMessage, SteerState};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

/// Context for managing steering within an execution.
pub struct SteeringContext {
    pub(crate) state: Arc<RwLock<SteerState>>,
    steer_rx: mpsc::Receiver<SteerMessage>,
    steer_tx: mpsc::Sender<SteerMessage>,
    execution_id: Uuid,
}

impl SteeringContext {
    /// Create a new steering context for the given execution ID.
    pub fn new(execution_id: Uuid) -> Self {
        let (steer_tx, steer_rx) = mpsc::channel(16);
        Self {
            state: Arc::new(RwLock::new(SteerState::Running)),
            steer_rx,
            steer_tx,
            execution_id,
        }
    }

    /// Get a handle to control this steering context.
    pub fn handle(&self) -> SteerHandle {
        SteerHandle::new(self.steer_tx.clone(), self.execution_id)
    }

    /// Check if the execution has been aborted.
    pub async fn is_aborted(&self) -> bool {
        matches!(*self.state.read().await, SteerState::Aborted)
    }

    /// Check for steering messages before executing a tool.
    pub async fn check_before_tool(&mut self) -> Result<SteerDecision, SteerError> {
        // First check current state
        if self.is_aborted().await {
            return Ok(SteerDecision::Abort(None));
        }

        // Try to receive a message non-blocking
        match self.steer_rx.try_recv() {
            Ok(SteerMessage::Abort { reason }) => {
                *self.state.write().await = SteerState::Aborted;
                Ok(SteerDecision::Abort(reason))
            }
            Ok(SteerMessage::SkipTool { tool_call_id }) => {
                Ok(SteerDecision::Skip(tool_call_id))
            }
            Ok(SteerMessage::UserText { content, .. }) => {
                *self.state.write().await = SteerState::Pending(SteerMessage::UserText {
                    content,
                    timestamp: Utc::now(),
                });
                Ok(SteerDecision::Continue)
            }
            Err(mpsc::error::TryRecvError::Empty) => Ok(SteerDecision::Continue),
            Err(mpsc::error::TryRecvError::Disconnected) => Err(SteerError::ChannelClosed),
        }
    }

    /// Apply effect of steering after tool execution (e.g. inject user message).
    pub async fn apply_after_tool(&self) -> Option<String> {
        let mut state = self.state.write().await;
        if let SteerState::Pending(SteerMessage::UserText { content, .. }) = &*state {
            let content = content.clone();
            *state = SteerState::Running; // Reset to running after applying
            Some(content)
        } else {
            None
        }
    }
}
