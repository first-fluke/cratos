use super::config::WhatsAppBusinessConfig;
use super::types::{ApiResponse, WebhookMessage, WhatsAppBusinessWebhook};
use crate::error::{Error, Result};
use crate::message::{ChannelAdapter, ChannelType, NormalizedMessage, OutgoingMessage};

use serde::Serialize;

use tracing::{debug, info};

/// WhatsApp Business API adapter
pub struct WhatsAppBusinessAdapter {
    config: WhatsAppBusinessConfig,
    client: reqwest::Client,
}

impl WhatsAppBusinessAdapter {
    /// Create a new WhatsApp Business adapter
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(config: WhatsAppBusinessConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Network(format!("Failed to create HTTP client: {e}")))?;

        info!("WhatsApp Business API adapter initialized");
        info!("Phone Number ID: {}", config.phone_number_id);

        Ok(Self { config, client })
    }

    /// Create from environment
    pub fn from_env() -> Result<Self> {
        let config = WhatsAppBusinessConfig::from_env()?;
        Self::new(config)
    }

    /// Verify webhook (for initial webhook setup)
    pub fn verify_webhook(&self, mode: &str, token: &str, challenge: &str) -> Option<String> {
        if mode == "subscribe" && token == self.config.webhook_verify_token {
            info!("WhatsApp webhook verified");
            Some(challenge.to_string())
        } else {
            None
        }
    }

    /// Check if a phone number is allowed
    pub fn is_number_allowed(&self, number: &str) -> bool {
        if self.config.allowed_numbers.is_empty() {
            return true;
        }
        let normalized = number.replace("+", "").replace("-", "").replace(" ", "");
        self.config.allowed_numbers.iter().any(|allowed| {
            let norm_allowed = allowed.replace("+", "").replace("-", "").replace(" ", "");
            normalized.contains(&norm_allowed) || norm_allowed.contains(&normalized)
        })
    }

    /// Extract messages from webhook payload
    pub fn extract_messages(
        &self,
        webhook: &WhatsAppBusinessWebhook,
    ) -> Vec<(String, WebhookMessage)> {
        let mut messages = Vec::new();

        for entry in &webhook.entry {
            for change in &entry.changes {
                if change.field != "messages" {
                    continue;
                }

                // Get sender name from contacts
                let sender_name = change
                    .value
                    .contacts
                    .first()
                    .and_then(|c| c.profile.as_ref())
                    .map(|p| p.name.clone())
                    .unwrap_or_default();

                for msg in &change.value.messages {
                    if msg.message_type == "text" {
                        messages.push((sender_name.clone(), msg.clone()));
                    }
                }
            }
        }

        messages
    }

    /// Convert webhook message to normalized message
    pub fn normalize_webhook_message(
        &self,
        sender_name: &str,
        msg: &WebhookMessage,
    ) -> Option<NormalizedMessage> {
        // Check if number is allowed
        if !self.is_number_allowed(&msg.from) {
            debug!(from = %msg.from, "Number not allowed");
            return None;
        }

        let text = msg.text.as_ref()?.body.clone();
        if text.is_empty() {
            return None;
        }

        let mut normalized =
            NormalizedMessage::new(ChannelType::WhatsApp, &msg.from, &msg.from, &msg.id, &text);

        if !sender_name.is_empty() {
            normalized = normalized.with_user_name(sender_name);
        }

        Some(normalized)
    }



    /// Mark message as read
    pub async fn mark_as_read(&self, message_id: &str) -> Result<()> {
        let url = self.config.messages_url();

        #[derive(Serialize)]
        struct ReadRequest<'a> {
            messaging_product: &'static str,
            status: &'static str,
            message_id: &'a str,
        }

        self.client
            .post(&url)
            .bearer_auth(&self.config.access_token)
            .json(&ReadRequest {
                messaging_product: "whatsapp",
                status: "read",
                message_id,
            })
            .send()
            .await
            .map_err(|e| Error::WhatsApp(format!("Failed to mark as read: {}", e)))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ChannelAdapter for WhatsAppBusinessAdapter {
    fn channel_type(&self) -> ChannelType {
        ChannelType::WhatsApp
    }

    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String> {
        let url = self.config.messages_url();

        #[derive(Serialize)]
        struct SendRequest<'a> {
            messaging_product: &'static str,
            to: &'a str,
            #[serde(rename = "type")]
            message_type: &'static str,
            text: TextBody<'a>,
        }

        #[derive(Serialize)]
        struct TextBody<'a> {
            body: &'a str,
        }

        let request = SendRequest {
            messaging_product: "whatsapp",
            to: channel_id,
            message_type: "text",
            text: TextBody {
                body: &message.text,
            },
        };

        let resp: ApiResponse = self
            .client
            .post(&url)
            .bearer_auth(&self.config.access_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::WhatsApp(format!("Failed to send message: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::WhatsApp(format!("Invalid API response: {}", e)))?;

        if let Some(error) = resp.error {
            return Err(Error::WhatsApp(format!(
                "API error {}: {}",
                error.code, error.message
            )));
        }

        let message_id = resp
            .messages
            .and_then(|m| m.first().map(|msg| msg.id.clone()))
            .unwrap_or_default();

        Ok(message_id)
    }

    async fn edit_message(
        &self,
        _channel_id: &str,
        _message_id: &str,
        _message: OutgoingMessage,
    ) -> Result<()> {
        Err(Error::NotEnabled(
            "WhatsApp doesn't support message editing".to_string(),
        ))
    }

    async fn delete_message(&self, _channel_id: &str, _message_id: &str) -> Result<()> {
        Err(Error::NotEnabled(
            "WhatsApp message deletion not supported via Business API".to_string(),
        ))
    }

    async fn send_typing(&self, _channel_id: &str) -> Result<()> {
        // WhatsApp Business API doesn't have a typing indicator
        Ok(())
    }
}
