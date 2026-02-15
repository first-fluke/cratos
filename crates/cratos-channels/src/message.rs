//! Message - Normalized message types
//!
//! This module provides a normalized message format that abstracts
//! the differences between various messaging platforms (Telegram, Slack, etc.).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Channel type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    /// Telegram
    Telegram,
    /// Slack
    Slack,
    /// Discord
    Discord,
    /// WhatsApp (Baileys or Business API)
    WhatsApp,
    /// Matrix (decentralized messaging)
    Matrix,
    /// Voice (local audio)
    Voice,
    /// CLI (command line)
    Cli,
    /// API (direct HTTP)
    Api,
}

impl ChannelType {
    /// Get the string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Telegram => "telegram",
            Self::Slack => "slack",
            Self::Discord => "discord",
            Self::WhatsApp => "whatsapp",
            Self::Matrix => "matrix",
            Self::Voice => "voice",
            Self::Cli => "cli",
            Self::Api => "api",
        }
    }
}

impl std::fmt::Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Attachment type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    /// Image file
    Image,
    /// Document file
    Document,
    /// Audio file
    Audio,
    /// Video file
    Video,
    /// Voice message
    Voice,
    /// Other/unknown
    Other,
}

/// An attachment in a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    /// Attachment type
    pub attachment_type: AttachmentType,
    /// File name (if available)
    pub file_name: Option<String>,
    /// MIME type (if available)
    pub mime_type: Option<String>,
    /// File size in bytes (if available)
    pub file_size: Option<u64>,
    /// URL to download the file (if available)
    pub url: Option<String>,
    /// Raw file ID from the platform
    pub file_id: Option<String>,
}

impl Attachment {
    /// Create a new image attachment
    #[must_use]
    pub fn image(file_id: impl Into<String>) -> Self {
        Self {
            attachment_type: AttachmentType::Image,
            file_name: None,
            mime_type: Some("image/jpeg".to_string()),
            file_size: None,
            url: None,
            file_id: Some(file_id.into()),
        }
    }

    /// Create a new document attachment
    #[must_use]
    pub fn document(file_id: impl Into<String>, file_name: Option<String>) -> Self {
        Self {
            attachment_type: AttachmentType::Document,
            file_name,
            mime_type: None,
            file_size: None,
            url: None,
            file_id: Some(file_id.into()),
        }
    }
}

/// A normalized incoming message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedMessage {
    /// Internal message ID
    pub id: Uuid,
    /// Channel type
    pub channel_type: ChannelType,
    /// Channel identifier (chat ID, channel ID, etc.)
    pub channel_id: String,
    /// User identifier
    pub user_id: String,
    /// User display name (if available)
    pub user_name: Option<String>,
    /// Thread/reply identifier (if applicable)
    pub thread_id: Option<String>,
    /// Original message ID from the platform
    pub message_id: String,
    /// Message text content
    pub text: String,
    /// Attachments
    pub attachments: Vec<Attachment>,
    /// Message timestamp
    pub timestamp: DateTime<Utc>,
    /// Whether this is a reply to the bot
    pub is_reply: bool,
    /// Whether this is a command (starts with /)
    pub is_command: bool,
    /// Extracted command name (if is_command)
    pub command: Option<String>,
    /// Command arguments (if is_command)
    pub command_args: Option<String>,
    /// Raw platform-specific data
    #[serde(skip)]
    pub raw_data: Option<serde_json::Value>,
}

impl NormalizedMessage {
    /// Create a new normalized message
    #[must_use]
    pub fn new(
        channel_type: ChannelType,
        channel_id: impl Into<String>,
        user_id: impl Into<String>,
        message_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        let text = text.into();
        let (is_command, command, command_args) = Self::parse_command(&text);

        Self {
            id: Uuid::new_v4(),
            channel_type,
            channel_id: channel_id.into(),
            user_id: user_id.into(),
            user_name: None,
            thread_id: None,
            message_id: message_id.into(),
            text,
            attachments: Vec::new(),
            timestamp: Utc::now(),
            is_reply: false,
            is_command,
            command,
            command_args,
            raw_data: None,
        }
    }

    /// Parse command from text
    fn parse_command(text: &str) -> (bool, Option<String>, Option<String>) {
        if text.starts_with('/') {
            let parts: Vec<&str> = text.splitn(2, ' ').collect();
            let command = parts[0].trim_start_matches('/').to_string();
            let args = parts.get(1).map(|s| s.to_string());
            (true, Some(command), args)
        } else {
            (false, None, None)
        }
    }

    /// Set the user name
    #[must_use]
    pub fn with_user_name(mut self, name: impl Into<String>) -> Self {
        self.user_name = Some(name.into());
        self
    }

    /// Set the thread ID
    #[must_use]
    pub fn with_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Mark as a reply
    #[must_use]
    pub fn as_reply(mut self) -> Self {
        self.is_reply = true;
        self
    }

    /// Add an attachment
    #[must_use]
    pub fn with_attachment(mut self, attachment: Attachment) -> Self {
        self.attachments.push(attachment);
        self
    }

    /// Set raw data
    #[must_use]
    pub fn with_raw_data(mut self, data: serde_json::Value) -> Self {
        self.raw_data = Some(data);
        self
    }

    /// Check if message has text content
    #[must_use]
    pub fn has_text(&self) -> bool {
        !self.text.is_empty()
    }

    /// Check if message has attachments
    #[must_use]
    pub fn has_attachments(&self) -> bool {
        !self.attachments.is_empty()
    }

    /// Get the effective text (command args if command, otherwise full text)
    #[must_use]
    pub fn effective_text(&self) -> &str {
        if self.is_command {
            self.command_args.as_deref().unwrap_or("")
        } else {
            &self.text
        }
    }
}

/// A normalized outgoing message
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutgoingMessage {
    /// Text content
    pub text: String,
    /// Whether to parse as markdown
    pub parse_markdown: bool,
    /// Reply to message ID (platform-specific)
    pub reply_to: Option<String>,
    /// Thread ID to post in
    pub thread_id: Option<String>,
    /// Inline keyboard buttons (for platforms that support it)
    pub buttons: Vec<MessageButton>,
}

impl OutgoingMessage {
    /// Create a simple text message
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            text: content.into(),
            ..Default::default()
        }
    }

    /// Create a markdown message
    #[must_use]
    pub fn markdown(content: impl Into<String>) -> Self {
        Self {
            text: content.into(),
            parse_markdown: true,
            ..Default::default()
        }
    }

    /// Set reply to message
    #[must_use]
    pub fn reply_to(mut self, message_id: impl Into<String>) -> Self {
        self.reply_to = Some(message_id.into());
        self
    }

    /// Set thread ID
    #[must_use]
    pub fn in_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Add a button
    #[must_use]
    pub fn with_button(mut self, button: MessageButton) -> Self {
        self.buttons.push(button);
        self
    }

    /// Add a row of buttons
    #[must_use]
    pub fn with_buttons(mut self, buttons: Vec<MessageButton>) -> Self {
        self.buttons.extend(buttons);
        self
    }
}

/// Outgoing attachment for sending files/media through channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingAttachment {
    /// File name
    pub filename: String,
    /// MIME type
    pub mime_type: String,
    /// Base64-encoded data
    pub data: String,
    /// Attachment type (image, document, audio, etc.)
    pub attachment_type: AttachmentType,
    /// Optional caption/description
    pub caption: Option<String>,
}

impl OutgoingAttachment {
    /// Create from raw bytes
    #[must_use]
    pub fn from_bytes(
        filename: impl Into<String>,
        mime_type: impl Into<String>,
        data: &[u8],
    ) -> Self {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let mime = mime_type.into();
        let attachment_type = Self::infer_type(&mime);
        Self {
            filename: filename.into(),
            mime_type: mime,
            data: STANDARD.encode(data),
            attachment_type,
            caption: None,
        }
    }

    /// Create an image attachment
    #[must_use]
    pub fn image(filename: impl Into<String>, mime_type: impl Into<String>, data: &[u8]) -> Self {
        use base64::{engine::general_purpose::STANDARD, Engine};
        Self {
            filename: filename.into(),
            mime_type: mime_type.into(),
            data: STANDARD.encode(data),
            attachment_type: AttachmentType::Image,
            caption: None,
        }
    }

    /// Create a document attachment
    #[must_use]
    pub fn document(
        filename: impl Into<String>,
        mime_type: impl Into<String>,
        data: &[u8],
    ) -> Self {
        use base64::{engine::general_purpose::STANDARD, Engine};
        Self {
            filename: filename.into(),
            mime_type: mime_type.into(),
            data: STANDARD.encode(data),
            attachment_type: AttachmentType::Document,
            caption: None,
        }
    }

    /// Set caption
    #[must_use]
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Decode the base64 data to bytes
    pub fn decode_data(&self) -> Result<Vec<u8>, base64::DecodeError> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        STANDARD.decode(&self.data)
    }

    /// Infer attachment type from MIME type
    fn infer_type(mime: &str) -> AttachmentType {
        if mime.starts_with("image/") {
            AttachmentType::Image
        } else if mime.starts_with("audio/") {
            AttachmentType::Audio
        } else if mime.starts_with("video/") {
            AttachmentType::Video
        } else {
            AttachmentType::Document
        }
    }
}

/// A button in a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageButton {
    /// Button text
    pub text: String,
    /// Callback data (sent when button is pressed)
    pub callback_data: Option<String>,
    /// URL to open when clicked
    pub url: Option<String>,
}

impl MessageButton {
    /// Create a callback button
    #[must_use]
    pub fn callback(text: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            callback_data: Some(data.into()),
            url: None,
        }
    }

    /// Create a URL button
    #[must_use]
    pub fn link(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            callback_data: None,
            url: Some(url.into()),
        }
    }
}

/// Trait for channel adapters
#[async_trait::async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// Get the channel type
    fn channel_type(&self) -> ChannelType;

    /// Send a message to a channel
    async fn send_message(
        &self,
        channel_id: &str,
        message: OutgoingMessage,
    ) -> crate::Result<String>;

    /// Edit a previously sent message
    async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        message: OutgoingMessage,
    ) -> crate::Result<()>;

    /// Delete a message
    async fn delete_message(&self, channel_id: &str, message_id: &str) -> crate::Result<()>;

    /// Send a typing indicator
    async fn send_typing(&self, channel_id: &str) -> crate::Result<()>;

    /// Send a file/attachment to a channel
    ///
    /// Default implementation sends a text fallback message.
    /// Channels should override this to send actual files.
    async fn send_attachment(
        &self,
        channel_id: &str,
        attachment: OutgoingAttachment,
        reply_to: Option<&str>,
    ) -> crate::Result<String> {
        // Default: send text fallback
        let caption = attachment.caption.as_deref().unwrap_or("").to_string();
        let text = if caption.is_empty() {
            format!("[Attachment: {}]", attachment.filename)
        } else {
            format!("[Attachment: {}] {}", attachment.filename, caption)
        };
        let mut msg = OutgoingMessage::text(text);
        if let Some(reply) = reply_to {
            msg = msg.reply_to(reply);
        }
        self.send_message(channel_id, msg).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalized_message() {
        let msg =
            NormalizedMessage::new(ChannelType::Telegram, "123", "456", "msg_1", "Hello world");

        assert_eq!(msg.channel_type, ChannelType::Telegram);
        assert_eq!(msg.text, "Hello world");
        assert!(!msg.is_command);
    }

    #[test]
    fn test_command_parsing() {
        let msg = NormalizedMessage::new(
            ChannelType::Telegram,
            "123",
            "456",
            "msg_1",
            "/help with args",
        );

        assert!(msg.is_command);
        assert_eq!(msg.command, Some("help".to_string()));
        assert_eq!(msg.command_args, Some("with args".to_string()));
    }

    #[test]
    fn test_outgoing_message() {
        let msg = OutgoingMessage::markdown("**Hello**")
            .reply_to("msg_1")
            .with_button(MessageButton::callback("Click me", "action:click"));

        assert!(msg.parse_markdown);
        assert_eq!(msg.reply_to, Some("msg_1".to_string()));
        assert_eq!(msg.buttons.len(), 1);
    }

    #[test]
    fn test_channel_type_display() {
        assert_eq!(ChannelType::Telegram.to_string(), "telegram");
        assert_eq!(ChannelType::Slack.to_string(), "slack");
    }

    #[test]
    fn test_outgoing_attachment() {
        let data = b"test image data";
        let att =
            OutgoingAttachment::image("test.png", "image/png", data).with_caption("Test caption");

        assert_eq!(att.filename, "test.png");
        assert_eq!(att.mime_type, "image/png");
        assert_eq!(att.attachment_type, AttachmentType::Image);
        assert_eq!(att.caption, Some("Test caption".to_string()));

        // Verify base64 encoding/decoding round-trip
        let decoded = att.decode_data().unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_outgoing_attachment_type_inference() {
        let data = b"test";

        let img = OutgoingAttachment::from_bytes("test.png", "image/png", data);
        assert_eq!(img.attachment_type, AttachmentType::Image);

        let audio = OutgoingAttachment::from_bytes("test.mp3", "audio/mpeg", data);
        assert_eq!(audio.attachment_type, AttachmentType::Audio);

        let video = OutgoingAttachment::from_bytes("test.mp4", "video/mp4", data);
        assert_eq!(video.attachment_type, AttachmentType::Video);

        let doc = OutgoingAttachment::from_bytes("test.pdf", "application/pdf", data);
        assert_eq!(doc.attachment_type, AttachmentType::Document);
    }
}
