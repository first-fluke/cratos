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
use matrix_sdk::{
    ruma::{
        events::room::message::{
            MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent,
            TextMessageEventContent,
        },
        RoomId,
    },
    Client,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

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
    "Cratos Bot".to_string()
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

        // Login with password
        let username = self
            .config
            .user_id
            .trim_start_matches('@')
            .split(':')
            .next()
            .unwrap_or(&self.config.user_id);

        client
            .matrix_auth()
            .login_username(username, &self.config.password)
            .initial_device_display_name(&self.config.device_name)
            .await
            .map_err(|e| Error::Network(format!("Failed to login: {e}")))?;

        info!("Matrix: Logged in as {}", self.config.user_id);

        // Store client
        let mut client_guard = self.client.write().await;
        *client_guard = Some(client);

        Ok(())
    }

    /// Check if a room is allowed
    pub fn is_room_allowed(&self, room_id: &str) -> bool {
        if self.config.allowed_rooms.is_empty() {
            return true;
        }
        self.config.allowed_rooms.iter().any(|r| r == room_id)
    }

    /// Get the Matrix client
    async fn get_client(&self) -> Result<Client> {
        let client_guard = self.client.read().await;
        client_guard
            .clone()
            .ok_or_else(|| Error::Network("Matrix client not connected".to_string()))
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
            _ => return None, // Only handle text messages for now
        };

        let user_id = event.sender.to_string();
        let event_id = event.event_id.to_string();

        // Thread ID from relates_to if available
        let thread_id = event.content.relates_to.as_ref().and_then(|r| match r {
            matrix_sdk::ruma::events::room::message::Relation::Thread(t) => {
                Some(t.event_id.to_string())
            }
            _ => None,
        });

        let mut msg =
            NormalizedMessage::new(ChannelType::Matrix, room_id_str, user_id, event_id, text);

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
}

#[async_trait]
impl ChannelAdapter for MatrixAdapter {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Matrix
    }

    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String> {
        let client = self.get_client().await?;

        let room_id = <&RoomId>::try_from(channel_id)
            .map_err(|e| Error::Network(format!("Invalid room ID: {e}")))?;

        let room = client
            .get_room(room_id)
            .ok_or_else(|| Error::Network(format!("Room not found: {channel_id}")))?;

        // Build message content
        let content = if message.parse_markdown {
            RoomMessageEventContent::text_html(&message.text, &message.text)
        } else {
            RoomMessageEventContent::text_plain(&message.text)
        };

        // Send message
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
        // Note: Matrix message editing requires the original event, which we don't have.
        // As a workaround, we send a new message mentioning the edit.
        // For proper edit support, we'd need to store/retrieve original events.
        warn!(
            "Matrix: Edit not fully supported, sending new message instead for {}",
            message_id
        );

        let _ = self
            .send_message(channel_id, OutgoingMessage::text(format!("(edit) {}", message.text)))
            .await?;

        debug!("Matrix: Sent edit message for {}", message_id);
        Ok(())
    }

    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let client = self.get_client().await?;

        let room_id = <&RoomId>::try_from(channel_id)
            .map_err(|e| Error::Network(format!("Invalid room ID: {e}")))?;

        let room = client
            .get_room(room_id)
            .ok_or_else(|| Error::Network(format!("Room not found: {channel_id}")))?;

        let event_id = matrix_sdk::ruma::OwnedEventId::try_from(message_id)
            .map_err(|e| Error::Network(format!("Invalid event ID: {e}")))?;

        room.redact(&event_id, None, None)
            .await
            .map_err(|e| Error::Network(format!("Failed to delete message: {e}")))?;

        debug!("Matrix: Deleted message {}", message_id);
        Ok(())
    }

    async fn send_typing(&self, channel_id: &str) -> Result<()> {
        let client = self.get_client().await?;

        let room_id = <&RoomId>::try_from(channel_id)
            .map_err(|e| Error::Network(format!("Invalid room ID: {e}")))?;

        let room = client
            .get_room(room_id)
            .ok_or_else(|| Error::Network(format!("Room not found: {channel_id}")))?;

        room.typing_notice(true)
            .await
            .map_err(|e| Error::Network(format!("Failed to send typing: {e}")))?;

        debug!("Matrix: Sent typing indicator to {}", channel_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_structure() {
        let config = MatrixConfig {
            homeserver_url: "https://matrix.org".to_string(),
            user_id: "@bot:matrix.org".to_string(),
            password: "secret".to_string(),
            device_name: "Test Bot".to_string(),
            allowed_rooms: vec!["!room:matrix.org".to_string()],
        };

        assert_eq!(config.homeserver_url, "https://matrix.org");
        assert!(config.is_room_allowed_check("!room:matrix.org"));
        assert!(!config.is_room_allowed_check("!other:matrix.org"));
    }

    #[test]
    fn test_empty_allowed_rooms() {
        let config = MatrixConfig {
            homeserver_url: "https://matrix.org".to_string(),
            user_id: "@bot:matrix.org".to_string(),
            password: "secret".to_string(),
            device_name: "Test Bot".to_string(),
            allowed_rooms: vec![],
        };

        // Empty allowed_rooms means all rooms are allowed
        assert!(config.is_room_allowed_check("!any:matrix.org"));
    }

    impl MatrixConfig {
        fn is_room_allowed_check(&self, room_id: &str) -> bool {
            if self.allowed_rooms.is_empty() {
                return true;
            }
            self.allowed_rooms.iter().any(|r| r == room_id)
        }
    }
}
