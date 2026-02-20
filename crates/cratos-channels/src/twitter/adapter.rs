use super::config::TwitterConfig;
use crate::error::{Error, Result};
use crate::message::{ChannelAdapter, ChannelType, OutgoingAttachment, OutgoingMessage};

/// Twitter adapter (placeholder for now)
pub struct TwitterAdapter {
    pub(crate) _config: TwitterConfig,
    pub(crate) _client: reqwest::Client,
}

impl TwitterAdapter {
    pub fn new(config: TwitterConfig) -> Self {
        Self {
            _config: config,
            _client: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let config = TwitterConfig::from_env()?;
        Ok(Self::new(config))
    }
}

#[async_trait::async_trait]
impl ChannelAdapter for TwitterAdapter {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Twitter
    }

    async fn send_message(&self, _channel_id: &str, _message: OutgoingMessage) -> Result<String> {
        Err(Error::Twitter(
            "Sending messages not implemented yet".to_string(),
        ))
    }

    async fn edit_message(
        &self,
        _channel_id: &str,
        _message_id: &str,
        _message: OutgoingMessage,
    ) -> Result<()> {
        Err(Error::Twitter(
            "Editing messages not implemented yet".to_string(),
        ))
    }

    async fn delete_message(&self, _channel_id: &str, _message_id: &str) -> Result<()> {
        Err(Error::Twitter(
            "Deleting messages not implemented yet".to_string(),
        ))
    }

    async fn send_typing(&self, _channel_id: &str) -> Result<()> {
        Ok(()) // No-op for Twitter typing
    }

    async fn send_attachment(
        &self,
        _channel_id: &str,
        _attachment: OutgoingAttachment,
        _reply_to: Option<&str>,
    ) -> Result<String> {
        Err(Error::Twitter(
            "Sending attachments not implemented yet".to_string(),
        ))
    }
}
