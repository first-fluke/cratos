//! Message types for LLM conversations
//!
//! This module defines the core message types used in LLM conversations.

use crate::ToolCall;
use base64::Engine;
use serde::{Deserialize, Serialize};

/// Role in a conversation message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message (instructions)
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool response
    Tool,
}

impl MessageRole {
    /// Returns the string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

/// Inline image content attached to a message
#[derive(Clone, Serialize, Deserialize)]
pub struct ImageContent {
    /// MIME type (e.g., "image/jpeg", "image/png")
    pub mime_type: String,
    /// Raw image bytes
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
}

impl std::fmt::Debug for ImageContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageContent")
            .field("mime_type", &self.mime_type)
            .field("data_len", &self.data.len())
            .finish()
    }
}

impl ImageContent {
    /// Create a new image content
    #[must_use]
    pub fn new(mime_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            mime_type: mime_type.into(),
            data,
        }
    }

    /// Return base64-encoded data
    #[must_use]
    pub fn base64_data(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(&self.data)
    }

    /// Return a data URI (data:image/jpeg;base64,...)
    #[must_use]
    pub fn data_uri(&self) -> String {
        format!("data:{};base64,{}", self.mime_type, self.base64_data())
    }
}

mod base64_bytes {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(data: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(data))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(serde::de::Error::custom)
    }
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// Tool call ID (for tool responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Name (for tool calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tool calls (for assistant messages requesting tool use)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// Inline images (for multimodal user messages)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageContent>,
}

impl Message {
    /// Create a system message
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: Vec::new(),
            images: Vec::new(),
        }
    }

    /// Create a user message
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: Vec::new(),
            images: Vec::new(),
        }
    }

    /// Create a user message with images
    #[must_use]
    pub fn user_with_images(content: impl Into<String>, images: Vec<ImageContent>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: Vec::new(),
            images,
        }
    }

    /// Create an assistant message
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: Vec::new(),
            images: Vec::new(),
        }
    }

    /// Create an assistant message with tool calls
    #[must_use]
    pub fn assistant_with_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls,
            images: Vec::new(),
        }
    }

    /// Create a tool response message
    #[must_use]
    pub fn tool_response(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            name: None,
            tool_calls: Vec::new(),
            images: Vec::new(),
        }
    }

    /// Create a tool response message with tool name
    #[must_use]
    pub fn tool_response_named(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            name: Some(name.into()),
            tool_calls: Vec::new(),
            images: Vec::new(),
        }
    }

    /// Check if this message has images
    #[must_use]
    pub fn has_images(&self) -> bool {
        !self.images.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let system = Message::system("You are a helpful assistant");
        assert_eq!(system.role, MessageRole::System);

        let user = Message::user("Hello!");
        assert_eq!(user.role, MessageRole::User);

        let assistant = Message::assistant("Hi there!");
        assert_eq!(assistant.role, MessageRole::Assistant);

        let tool = Message::tool_response("call_123", r#"{"result": "ok"}"#);
        assert_eq!(tool.role, MessageRole::Tool);
        assert_eq!(tool.tool_call_id, Some("call_123".to_string()));
    }

    #[test]
    fn test_message_role_as_str() {
        assert_eq!(MessageRole::System.as_str(), "system");
        assert_eq!(MessageRole::User.as_str(), "user");
        assert_eq!(MessageRole::Assistant.as_str(), "assistant");
        assert_eq!(MessageRole::Tool.as_str(), "tool");
    }
}
