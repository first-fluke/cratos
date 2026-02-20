//! Chat WebSocket protocol definitions

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Chat message from client
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Send a chat message
    Chat {
        text: String,
        persona: Option<String>,
    },
    /// Send an E2E encrypted chat message
    EncryptedChat {
        /// E2E session ID (from /api/v1/sessions/init-e2e)
        e2e_session_id: Uuid,
        /// Base64-encoded nonce
        nonce: String,
        /// Base64-encoded ciphertext
        ciphertext: String,
    },
    /// Activate E2E encryption for this WS connection
    ActivateE2e {
        /// E2E session ID (from /api/v1/sessions/init-e2e)
        e2e_session_id: Uuid,
    },
    /// Request status
    Status,
    /// Cancel current execution
    Cancel { execution_id: Option<Uuid> },
    /// Ping for keepalive
    Ping,
}

/// Chat message to client
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Chat response (may be streaming)
    ChatResponse {
        execution_id: Uuid,
        text: String,
        is_final: bool,
        persona: String,
    },
    /// Status update
    Status {
        connected: bool,
        active_executions: usize,
        persona: String,
    },
    /// Tool call notification
    ToolCall {
        execution_id: Uuid,
        tool_name: String,
        status: String,
    },
    /// Artifact (file, image, etc.) generated during execution
    Artifact {
        execution_id: Uuid,
        filename: String,
        mime_type: String,
        /// Base64-encoded data
        data: String,
    },
    /// Error message
    Error {
        message: String,
        code: Option<String>,
    },
    /// Pong response
    Pong,
    /// Connection established
    Connected { session_id: Uuid },
}
