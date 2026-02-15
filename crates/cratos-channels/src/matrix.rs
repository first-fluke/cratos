//! Matrix - matrix-sdk adapter
//!
//! This module provides the Matrix messaging adapter using the matrix-sdk library.
//! Matrix is an open-source, decentralized messaging protocol.
//!
//! ## Features
//!
//! - Connect to any Matrix homeserver (matrix.org, Element, self-hosted)
//! - Room-based messaging with thread support
//! - File/media attachment handling
//!
//! ## Configuration
//!
//! ```toml
//! [channels.matrix]
//! enabled = true
//! homeserver_url = "https://matrix.org"
//! user_id = "@bot:matrix.org"
//! password = "${MATRIX_PASSWORD}"
//! allowed_rooms = []  # empty = allow all
//! ```

use crate::error::{Error, Result};
use crate::message::{ChannelAdapter, ChannelType, NormalizedMessage, OutgoingMessage};
use async_trait::async_trait;
use cratos_core::{Orchestrator, OrchestratorInput};
use matrix_sdk::{
    config::SyncSettings,
    room::Room,
    ruma::{
        events::room::message::{
            AudioMessageEventContent, FileMessageEventContent, ImageMessageEventContent,
            MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent,
            TextMessageEventContent, VideoMessageEventContent,
        },
        OwnedEventId, OwnedMxcUri, RoomId,
    },
    Client,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Default device name for Matrix client
const DEFAULT_DEVICE_NAME: &str = "Cratos Bot";

/// Prefix for edit message workaround
const EDIT_MESSAGE_PREFIX: &str = "(edit) ";

/// Matrix adapter configuration
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixConfig {
    /// Matrix homeserver URL (e.g., "https://matrix.org")
    pub homeserver_url: String,

    /// Bot user ID (e.g., "@bot:matrix.org")
    pub user_id: String,

    /// Password for login
    pub password: String,

    /// Device display name
    #[serde(default = "default_device_name")]
    pub device_name: String,

    /// Allowed room IDs (empty = allow all)
    #[serde(default)]
    pub allowed_rooms: Vec<String>,
}

fn default_device_name() -> String {
    DEFAULT_DEVICE_NAME.to_string()
}

impl MatrixConfig {
    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let homeserver_url = std::env::var("MATRIX_HOMESERVER_URL")
            .map_err(|_| Error::Network("MATRIX_HOMESERVER_URL not set".to_string()))?;

        let user_id = std::env::var("MATRIX_USER_ID")
            .map_err(|_| Error::Network("MATRIX_USER_ID not set".to_string()))?;

        let password = std::env::var("MATRIX_PASSWORD")
            .map_err(|_| Error::Network("MATRIX_PASSWORD not set".to_string()))?;

        let allowed_rooms = std::env::var("MATRIX_ALLOWED_ROOMS")
            .map(|s| s.split(',').map(|r| r.trim().to_string()).collect())
            .unwrap_or_default();

        Ok(Self {
            homeserver_url,
            user_id,
            password,
            device_name: default_device_name(),
            allowed_rooms,
        })
    }

    /// Check if a room is allowed
    pub fn is_room_allowed(&self, room_id: &str) -> bool {
        self.allowed_rooms.is_empty() || self.allowed_rooms.iter().any(|r| r == room_id)
    }
}

/// Matrix messaging adapter
pub struct MatrixAdapter {
    config: MatrixConfig,
    client: Arc<RwLock<Option<Client>>>,
}

impl MatrixAdapter {
    /// Create a new Matrix adapter
    ///
    /// # Errors
    /// Returns an error if configuration is invalid.
    pub fn new(config: MatrixConfig) -> Result<Self> {
        Ok(Self {
            config,
            client: Arc::new(RwLock::new(None)),
        })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let config = MatrixConfig::from_env()?;
        Self::new(config)
    }

    /// Connect to the Matrix homeserver
    pub async fn connect(&self) -> Result<()> {
        let homeserver_url = url::Url::parse(&self.config.homeserver_url)
            .map_err(|e| Error::Network(format!("Invalid homeserver URL: {e}")))?;

        let client = Client::new(homeserver_url)
            .await
            .map_err(|e| Error::Network(format!("Failed to create Matrix client: {e}")))?;

        let username = self.extract_username();

        client
            .matrix_auth()
            .login_username(username, &self.config.password)
            .initial_device_display_name(&self.config.device_name)
            .await
            .map_err(|e| Error::Network(format!("Failed to login: {e}")))?;

        info!("Matrix: Logged in as {}", self.config.user_id);

        let mut client_guard = self.client.write().await;
        *client_guard = Some(client);

        Ok(())
    }

    /// Extract username from user_id (e.g., "@bot:matrix.org" -> "bot")
    fn extract_username(&self) -> &str {
        self.config
            .user_id
            .trim_start_matches('@')
            .split(':')
            .next()
            .unwrap_or(&self.config.user_id)
    }

    /// Check if a room is allowed
    pub fn is_room_allowed(&self, room_id: &str) -> bool {
        self.config.is_room_allowed(room_id)
    }

    /// Get the Matrix client (must be connected first)
    async fn get_client(&self) -> Result<Client> {
        self.client
            .read()
            .await
            .clone()
            .ok_or_else(|| Error::Network("Matrix client not connected".to_string()))
    }

    /// Get a room by ID
    async fn get_room(&self, channel_id: &str) -> Result<Room> {
        let client = self.get_client().await?;

        let room_id = <&RoomId>::try_from(channel_id)
            .map_err(|e| Error::Network(format!("Invalid room ID: {e}")))?;

        client
            .get_room(room_id)
            .ok_or_else(|| Error::Network(format!("Room not found: {channel_id}")))
    }

    /// Parse event ID from string
    fn parse_event_id(message_id: &str) -> Result<OwnedEventId> {
        OwnedEventId::try_from(message_id)
            .map_err(|e| Error::Network(format!("Invalid event ID: {e}")))
    }

    /// Normalize a Matrix message
    pub fn normalize_message(
        &self,
        room_id: &RoomId,
        event: &OriginalSyncRoomMessageEvent,
    ) -> Option<NormalizedMessage> {
        let room_id_str = room_id.to_string();

        if !self.is_room_allowed(&room_id_str) {
            debug!(
                "Matrix: Ignoring message from non-allowed room {}",
                room_id_str
            );
            return None;
        }

        let text = match &event.content.msgtype {
            MessageType::Text(TextMessageEventContent { body, .. }) => body.clone(),
            _ => return None,
        };

        let thread_id = self.extract_thread_id(event);

        let mut msg = NormalizedMessage::new(
            ChannelType::Matrix,
            room_id_str,
            event.sender.to_string(),
            event.event_id.to_string(),
            text,
        );

        if let Some(tid) = thread_id {
            msg = msg.with_thread(tid);
        }

        msg = msg.with_raw_data(serde_json::json!({
            "event_id": event.event_id.to_string(),
            "room_id": room_id.to_string(),
            "sender": event.sender.to_string(),
        }));

        Some(msg)
    }

    /// Extract thread ID from event's relates_to field
    fn extract_thread_id(&self, event: &OriginalSyncRoomMessageEvent) -> Option<String> {
        event.content.relates_to.as_ref().and_then(|r| match r {
            matrix_sdk::ruma::events::room::message::Relation::Thread(t) => {
                Some(t.event_id.to_string())
            }
            _ => None,
        })
    }

    /// Run the Matrix adapter event loop
    ///
    /// Connects to the homeserver, syncs events, and processes incoming messages.
    /// Runs until shutdown signal is received.
    ///
    /// # Arguments
    /// * `orchestrator` - AI orchestrator for message processing
    /// * `shutdown` - Cancellation token for graceful shutdown
    ///
    /// # Errors
    /// Returns error if connection fails or sync encounters an unrecoverable error.
    pub async fn run(
        self: Arc<Self>,
        orchestrator: Arc<Orchestrator>,
        shutdown: CancellationToken,
    ) -> crate::error::Result<()> {
        // Connect to homeserver
        self.connect().await?;
        info!("Matrix adapter connected to {}", self.config.homeserver_url);

        let client = self.get_client().await?;
        let adapter = self.clone();
        let orch = orchestrator.clone();
        let own_user_id = self.config.user_id.clone();

        // Register message handler
        client.add_event_handler(move |event: OriginalSyncRoomMessageEvent, room: Room| {
            let adapter = adapter.clone();
            let orch = orch.clone();
            let own_user_id = own_user_id.clone();

            async move {
                let room_id = room.room_id();

                // Normalize and filter message
                let Some(normalized) = adapter.normalize_message(room_id, &event) else {
                    return;
                };

                // Skip own messages
                if event.sender.as_str() == own_user_id {
                    return;
                }

                debug!(
                    room_id = %room_id,
                    sender = %event.sender,
                    "Processing Matrix message"
                );

                // Send typing indicator
                let _ = adapter.send_typing(room_id.as_str()).await;

                // Process with orchestrator
                let input = OrchestratorInput::new(
                    "matrix",
                    room_id.as_str(),
                    event.sender.as_str(),
                    &normalized.text,
                );

                match orch.process(input).await {
                    Ok(result) => {
                        let response = if result.response.is_empty() {
                            "Done.".to_string()
                        } else {
                            result.response
                        };

                        if let Err(e) = adapter
                            .send_message(
                                room_id.as_str(),
                                crate::message::OutgoingMessage::text(response),
                            )
                            .await
                        {
                            warn!(error = %e, "Failed to send Matrix response");
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Matrix orchestrator error");
                        let _ = adapter
                            .send_message(
                                room_id.as_str(),
                                crate::message::OutgoingMessage::text(
                                    "Sorry, I encountered an error.",
                                ),
                            )
                            .await;
                    }
                }
            }
        });

        // Sync loop
        let sync_settings = SyncSettings::default().timeout(std::time::Duration::from_secs(30));

        loop {
            tokio::select! {
                sync_result = client.sync_once(sync_settings.clone()) => {
                    match sync_result {
                        Ok(_) => {
                            debug!("Matrix sync completed");
                        }
                        Err(e) => {
                            warn!(error = %e, "Matrix sync error, retrying...");
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        }
                    }
                }
                _ = shutdown.cancelled() => {
                    info!("Matrix adapter shutting down");
                    break;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl ChannelAdapter for MatrixAdapter {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Matrix
    }

    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String> {
        let room = self.get_room(channel_id).await?;

        let content = if message.parse_markdown {
            RoomMessageEventContent::text_html(&message.text, &message.text)
        } else {
            RoomMessageEventContent::text_plain(&message.text)
        };

        let response = room
            .send(content)
            .await
            .map_err(|e| Error::Network(format!("Failed to send message: {e}")))?;

        let event_id = response.event_id.to_string();
        debug!("Matrix: Sent message {}", event_id);

        Ok(event_id)
    }

    async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        message: OutgoingMessage,
    ) -> Result<()> {
        // Matrix message editing requires the original event, which we don't store.
        // Workaround: send a new message with edit prefix.
        warn!(
            "Matrix: Edit not fully supported, sending new message instead for {}",
            message_id
        );

        let edit_text = format!("{}{}", EDIT_MESSAGE_PREFIX, message.text);
        let _ = self
            .send_message(channel_id, OutgoingMessage::text(edit_text))
            .await?;

        debug!("Matrix: Sent edit message for {}", message_id);
        Ok(())
    }

    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let room = self.get_room(channel_id).await?;
        let event_id = Self::parse_event_id(message_id)?;

        room.redact(&event_id, None, None)
            .await
            .map_err(|e| Error::Network(format!("Failed to delete message: {e}")))?;

        debug!("Matrix: Deleted message {}", message_id);
        Ok(())
    }

    async fn send_typing(&self, channel_id: &str) -> Result<()> {
        let room = self.get_room(channel_id).await?;

        room.typing_notice(true)
            .await
            .map_err(|e| Error::Network(format!("Failed to send typing: {e}")))?;

        debug!("Matrix: Sent typing indicator to {}", channel_id);
        Ok(())
    }

    async fn send_attachment(
        &self,
        channel_id: &str,
        attachment: crate::message::OutgoingAttachment,
        _reply_to: Option<&str>,
    ) -> Result<String> {
        use base64::Engine as _;

        let room = self.get_room(channel_id).await?;

        // Decode base64 attachment data
        let file_data = base64::engine::general_purpose::STANDARD
            .decode(&attachment.data)
            .map_err(|e| Error::Network(format!("Invalid base64 attachment data: {}", e)))?;

        // Upload media to Matrix homeserver
        let client = self.client.read().await;
        let client = client
            .as_ref()
            .ok_or_else(|| Error::Network("Not connected".to_string()))?;

        // Parse MIME type or default to application/octet-stream
        let mime_type: mime::Mime = attachment
            .mime_type
            .parse()
            .unwrap_or(mime::APPLICATION_OCTET_STREAM);

        let media_response = client
            .media()
            .upload(&mime_type, file_data, None)
            .await
            .map_err(|e| Error::Network(format!("Failed to upload media: {e}")))?;

        let mxc_uri: OwnedMxcUri = media_response.content_uri;

        // Create appropriate message content based on MIME type
        let content = match attachment
            .mime_type
            .split('/')
            .next()
            .unwrap_or("application")
        {
            "image" => {
                let mut img_content =
                    ImageMessageEventContent::plain(attachment.filename.clone(), mxc_uri);
                if let Some(caption) = &attachment.caption {
                    img_content.body = format!("{} - {}", attachment.filename, caption);
                }
                RoomMessageEventContent::new(MessageType::Image(img_content))
            }
            "video" => {
                let mut vid_content =
                    VideoMessageEventContent::plain(attachment.filename.clone(), mxc_uri);
                if let Some(caption) = &attachment.caption {
                    vid_content.body = format!("{} - {}", attachment.filename, caption);
                }
                RoomMessageEventContent::new(MessageType::Video(vid_content))
            }
            "audio" => {
                let mut audio_content =
                    AudioMessageEventContent::plain(attachment.filename.clone(), mxc_uri);
                if let Some(caption) = &attachment.caption {
                    audio_content.body = format!("{} - {}", attachment.filename, caption);
                }
                RoomMessageEventContent::new(MessageType::Audio(audio_content))
            }
            _ => {
                let mut file_content =
                    FileMessageEventContent::plain(attachment.filename.clone(), mxc_uri);
                if let Some(caption) = &attachment.caption {
                    file_content.body = format!("{} - {}", attachment.filename, caption);
                }
                RoomMessageEventContent::new(MessageType::File(file_content))
            }
        };

        // Send the message
        let response = room
            .send(content)
            .await
            .map_err(|e| Error::Network(format!("Failed to send attachment: {e}")))?;

        let message_id = response.event_id.to_string();
        debug!(
            "Matrix: Sent attachment {} -> {}",
            attachment.filename, message_id
        );

        Ok(message_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config(allowed_rooms: Vec<String>) -> MatrixConfig {
        MatrixConfig {
            homeserver_url: "https://matrix.org".to_string(),
            user_id: "@bot:matrix.org".to_string(),
            password: "secret".to_string(),
            device_name: "Test Bot".to_string(),
            allowed_rooms,
        }
    }

    #[test]
    fn test_config_structure() {
        let config = create_test_config(vec!["!room:matrix.org".to_string()]);

        assert_eq!(config.homeserver_url, "https://matrix.org");
        assert!(config.is_room_allowed("!room:matrix.org"));
        assert!(!config.is_room_allowed("!other:matrix.org"));
    }

    #[test]
    fn test_empty_allowed_rooms_allows_all() {
        let config = create_test_config(vec![]);

        assert!(config.is_room_allowed("!any:matrix.org"));
        assert!(config.is_room_allowed("!another:example.com"));
    }

    #[test]
    fn test_extract_username() {
        let config = create_test_config(vec![]);
        let adapter = MatrixAdapter::new(config).expect("Failed to create adapter");

        assert_eq!(adapter.extract_username(), "bot");
    }

    #[test]
    fn test_default_device_name() {
        assert_eq!(default_device_name(), DEFAULT_DEVICE_NAME);
    }

    #[test]
    fn test_mime_type_parsing() {
        // Test MIME type parsing used in send_attachment
        let valid_mime: std::result::Result<mime::Mime, _> = "image/png".parse();
        assert!(valid_mime.is_ok());

        let invalid_mime: std::result::Result<mime::Mime, _> = "not-a-mime".parse();
        // Falls back to APPLICATION_OCTET_STREAM
        let fallback = invalid_mime.unwrap_or(mime::APPLICATION_OCTET_STREAM);
        assert_eq!(fallback, mime::APPLICATION_OCTET_STREAM);
    }

    #[test]
    fn test_media_type_classification() {
        // Test media type classification for message content type selection
        let test_cases = [
            ("image/jpeg", "image"),
            ("image/png", "image"),
            ("image/gif", "image"),
            ("video/mp4", "video"),
            ("video/webm", "video"),
            ("audio/mpeg", "audio"),
            ("audio/ogg", "audio"),
            ("application/pdf", "application"),
            ("text/plain", "text"),
        ];

        for (mime_type, expected_category) in test_cases {
            let category = mime_type.split('/').next().unwrap_or("application");
            assert_eq!(category, expected_category);
        }
    }

    #[test]
    fn test_attachment_body_with_caption() {
        let filename = "test.png";
        let caption = "Test caption";

        let body_with_caption = format!("{} - {}", filename, caption);
        assert_eq!(body_with_caption, "test.png - Test caption");
    }

    #[test]
    fn test_base64_decode_for_matrix_upload() {
        use base64::Engine as _;

        let test_data = b"Matrix file upload test data";
        let encoded = base64::engine::general_purpose::STANDARD.encode(test_data);
        let decoded = base64::engine::general_purpose::STANDARD.decode(&encoded);

        assert!(decoded.is_ok());
        assert_eq!(decoded.unwrap(), test_data);
    }

    #[test]
    fn test_user_id_parsing() {
        // Matrix user ID format: @localpart:domain
        let user_id = "@bot:matrix.org";
        let localpart = user_id
            .strip_prefix('@')
            .and_then(|s| s.split(':').next())
            .unwrap_or("unknown");

        assert_eq!(localpart, "bot");
    }
}
