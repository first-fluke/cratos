//! WebSocket Protocol Messages
//!
//! This module defines the client/server message types for the canvas WebSocket API.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::document::{CanvasBlock, CanvasDocument};

/// Messages sent from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Join a session
    Join {
        /// Session ID to join
        session_id: Uuid,
    },

    /// Leave the current session
    Leave,

    /// Update a block's content
    UpdateBlock {
        /// Block ID to update
        block_id: Uuid,
        /// New content
        content: String,
    },

    /// Add a new block
    AddBlock {
        /// Block to add
        block: CanvasBlock,
        /// Insert after this block ID (None = end of document)
        after_id: Option<Uuid>,
    },

    /// Delete a block
    DeleteBlock {
        /// Block ID to delete
        block_id: Uuid,
    },

    /// Move a block
    MoveBlock {
        /// Block ID to move
        block_id: Uuid,
        /// New position index
        new_index: usize,
    },

    /// Ask AI to process a prompt
    AskAi {
        /// User prompt
        prompt: String,
        /// Block IDs to include as context
        #[serde(default)]
        context_blocks: Vec<Uuid>,
        /// Target block ID for AI response (None = new block)
        target_block_id: Option<Uuid>,
    },

    /// Stop ongoing AI generation
    StopAi,

    /// Execute a code block
    ExecuteCode {
        /// Block ID containing code to execute
        block_id: Uuid,
    },

    /// Ping to keep connection alive
    Ping,

    /// A2UI Protocol - Client Message
    A2ui(crate::a2ui::A2uiClientMessage),
}

/// Messages sent from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// A2UI Protocol - Server Message
    A2ui(crate::a2ui::A2uiServerMessage),

    /// Welcome message with session state
    Welcome {
        /// Session ID
        session_id: Uuid,
        /// Current document state
        document: CanvasDocument,
    },

    /// Session state update (full sync)
    SessionState {
        /// Full document
        document: CanvasDocument,
    },

    /// A block was updated
    BlockUpdated {
        /// Block ID that was updated
        block_id: Uuid,
        /// New content
        content: String,
        /// Who made the update
        source: UpdateSource,
    },

    /// A block was added
    BlockAdded {
        /// The new block
        block: CanvasBlock,
        /// Index where it was inserted
        index: usize,
    },

    /// A block was deleted
    BlockDeleted {
        /// Block ID that was deleted
        block_id: Uuid,
    },

    /// A block was moved
    BlockMoved {
        /// Block ID that was moved
        block_id: Uuid,
        /// New index
        new_index: usize,
    },

    /// AI streaming response
    AiStreaming {
        /// Target block ID
        block_id: Uuid,
        /// Content chunk
        chunk: String,
        /// Whether this is the final chunk
        is_complete: bool,
    },

    /// AI response started
    AiStarted {
        /// Target block ID being updated
        block_id: Uuid,
    },

    /// AI response completed
    AiCompleted {
        /// Target block ID
        block_id: Uuid,
        /// Total tokens used
        tokens_used: Option<u32>,
    },

    /// AI error occurred
    AiError {
        /// Error message
        message: String,
    },

    /// Code execution started
    ExecutionStarted {
        /// Block ID being executed
        block_id: Uuid,
    },

    /// Code execution output
    ExecutionOutput {
        /// Block ID
        block_id: Uuid,
        /// Output text
        output: String,
        /// Whether this is stderr
        is_error: bool,
    },

    /// Code execution completed
    ExecutionCompleted {
        /// Block ID
        block_id: Uuid,
        /// Exit code
        exit_code: i32,
    },

    /// Error message
    Error {
        /// Error code
        code: String,
        /// Error message
        message: String,
    },

    /// Pong response to ping
    Pong,
}

/// Source of an update
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSource {
    /// Update from the current user
    User,
    /// Update from another user (collaboration)
    OtherUser,
    /// Update from AI
    Ai,
    /// Update from system
    System,
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected to any session
    Disconnected,
    /// Connecting to a session
    Connecting,
    /// Connected to a session
    Connected,
    /// Connection error
    Error,
}

impl ServerMessage {
    /// Create an error message
    #[must_use]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Create a welcome message
    #[must_use]
    pub fn welcome(session_id: Uuid, document: CanvasDocument) -> Self {
        Self::Welcome {
            session_id,
            document,
        }
    }

    /// Create a block updated message
    #[must_use]
    pub fn block_updated(block_id: Uuid, content: impl Into<String>, source: UpdateSource) -> Self {
        Self::BlockUpdated {
            block_id,
            content: content.into(),
            source,
        }
    }

    /// Create an AI streaming message
    #[must_use]
    pub fn ai_streaming(block_id: Uuid, chunk: impl Into<String>, is_complete: bool) -> Self {
        Self::AiStreaming {
            block_id,
            chunk: chunk.into(),
            is_complete,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage::Join {
            session_id: Uuid::nil(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"join\""));

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ClientMessage::Join { session_id } => {
                assert_eq!(session_id, Uuid::nil());
            }
            other => unreachable!("Expected Join message, got {:?}", other),
        }
    }

    #[test]
    fn test_server_message_error() {
        let msg = ServerMessage::error("not_found", "Session not found");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"code\":\"not_found\""));
    }

    #[test]
    fn test_update_source_serialization() {
        let source = UpdateSource::Ai;
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, "\"ai\"");

        let parsed: UpdateSource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, UpdateSource::Ai);
    }

    #[test]
    fn test_client_ask_ai_message() {
        let msg = ClientMessage::AskAi {
            prompt: "Explain this code".to_string(),
            context_blocks: vec![Uuid::new_v4()],
            target_block_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"ask_ai\""));
        assert!(json.contains("\"prompt\":\"Explain this code\""));
    }

    #[test]
    fn test_server_ai_streaming_message() {
        let block_id = Uuid::new_v4();
        let msg = ServerMessage::ai_streaming(block_id, "Hello ", false);

        match msg {
            ServerMessage::AiStreaming {
                block_id: id,
                chunk,
                is_complete,
            } => {
                assert_eq!(id, block_id);
                assert_eq!(chunk, "Hello ");
                assert!(!is_complete);
            }
            other => unreachable!("Expected AiStreaming message, got {:?}", other),
        }
    }
}
