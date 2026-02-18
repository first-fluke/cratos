use chrono::{DateTime, Utc};
use thiserror::Error;

/// Steering message sent by the user or system to control agent execution.
#[derive(Debug, Clone, PartialEq)]
pub enum SteerMessage {
    /// Inject a text message from the user into the conversation stream.
    UserText {
        /// The content of the user message.
        content: String,
        /// The timestamp when the message was created.
        timestamp: DateTime<Utc>,
    },
    /// Abort the current execution.
    Abort {
        /// Optional reason for aborting.
        reason: Option<String>,
    },
    /// Skip a specific tool execution.
    SkipTool {
        /// The ID of the tool call to skip.
        tool_call_id: String,
    },
    // Reprioritize deferred for V2
}

/// Current state of the steering mechanism.
#[derive(Debug, Clone, PartialEq)]
pub enum SteerState {
    /// Normal execution.
    Running,
    /// A steering message is pending processing.
    Pending(SteerMessage),
    /// Execution has been aborted.
    Aborted,
}

/// Decision made by the steering context before tool execution.
#[derive(Debug)]
pub enum SteerDecision {
    /// Continue with normal execution.
    Continue,
    /// Abort execution.
    Abort(Option<String>),
    /// Skip the current tool.
    Skip(String),
}

/// Errors related to steering operations.
#[derive(Error, Debug)]
pub enum SteerError {
    /// The steering channel has been closed.
    #[error("Steering channel closed")]
    ChannelClosed,
    /// The steering context is in an invalid state.
    #[error("Invalid steering state")]
    InvalidState,
}
