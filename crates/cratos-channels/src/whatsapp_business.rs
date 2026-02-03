//! WhatsApp Business API adapter
//!
//! This module provides the official WhatsApp Business Cloud API adapter.
//! Unlike the Baileys bridge, this is the official API and does not carry ban risks.
//!
//! # Setup
//!
//! 1. Create a Meta Business account
//! 2. Set up WhatsApp Business API in Meta Business Suite
//! 3. Get your Access Token, Phone Number ID, and Business Account ID
//! 4. Configure webhook for receiving messages

use crate::error::{Error, Result};
use crate::message::{ChannelAdapter, ChannelType, NormalizedMessage, OutgoingMessage};
use cratos_core::{Orchestrator, OrchestratorInput};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

/// Maximum length of text to log
const MAX_LOG_TEXT_LENGTH: usize = 50;

/// Sensitive patterns to mask
const SENSITIVE_PATTERNS: &[&str] = &[
    "password",
    "secret",
    "token",
    "api_key",
    "bearer",
    "credential",
    "private",
];

/// Mask sensitive text for logging
fn mask_for_logging(text: &str) -> String {
    let lower = text.to_lowercase();
    for pattern in SENSITIVE_PATTERNS {
        if lower.contains(pattern) {
            return "[REDACTED]".to_string();
        }
    }
    if text.len() > MAX_LOG_TEXT_LENGTH {
        format!("{}...", &text[..MAX_LOG_TEXT_LENGTH])
    } else {
        text.to_string()
    }
}

/// WhatsApp Business API configuration
#[derive(Debug, Clone, Deserialize)]
pub struct WhatsAppBusinessConfig {
    /// Access token (from Meta Business Suite)
    pub access_token: String,
    /// Phone Number ID (the bot's phone number ID)
    pub phone_number_id: String,
    /// Business Account ID
    pub business_account_id: String,
    /// Webhook verify token (for webhook verification)
    pub webhook_verify_token: String,
    /// Allowed phone numbers (empty = allow all)
    #[serde(default)]
    pub allowed_numbers: Vec<String>,
    /// API version (default: v18.0)
    #[serde(default = "default_api_version")]
    pub api_version: String,
}

fn default_api_version() -> String {
    "v18.0".to_string()
}

impl WhatsAppBusinessConfig {
    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let access_token = std::env::var("WHATSAPP_ACCESS_TOKEN")
            .map_err(|_| Error::WhatsApp("WHATSAPP_ACCESS_TOKEN not set".to_string()))?;

        let phone_number_id = std::env::var("WHATSAPP_PHONE_NUMBER_ID")
            .map_err(|_| Error::WhatsApp("WHATSAPP_PHONE_NUMBER_ID not set".to_string()))?;

        let business_account_id = std::env::var("WHATSAPP_BUSINESS_ACCOUNT_ID")
            .map_err(|_| Error::WhatsApp("WHATSAPP_BUSINESS_ACCOUNT_ID not set".to_string()))?;

        let webhook_verify_token = std::env::var("WHATSAPP_WEBHOOK_VERIFY_TOKEN")
            .unwrap_or_else(|_| "cratos_webhook_verify".to_string());

        let allowed_numbers: Vec<String> = std::env::var("WHATSAPP_ALLOWED_NUMBERS")
            .ok()
            .map(|s| s.split(',').map(|n| n.trim().to_string()).collect())
            .unwrap_or_default();

        let api_version =
            std::env::var("WHATSAPP_API_VERSION").unwrap_or_else(|_| default_api_version());

        Ok(Self {
            access_token,
            phone_number_id,
            business_account_id,
            webhook_verify_token,
            allowed_numbers,
            api_version,
        })
    }

    /// Create with required fields
    #[must_use]
    pub fn new(
        access_token: impl Into<String>,
        phone_number_id: impl Into<String>,
        business_account_id: impl Into<String>,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            phone_number_id: phone_number_id.into(),
            business_account_id: business_account_id.into(),
            webhook_verify_token: "cratos_webhook_verify".to_string(),
            allowed_numbers: Vec::new(),
            api_version: default_api_version(),
        }
    }

    /// Set webhook verify token
    #[must_use]
    pub fn with_webhook_verify_token(mut self, token: impl Into<String>) -> Self {
        self.webhook_verify_token = token.into();
        self
    }

    /// Set allowed numbers
    #[must_use]
    pub fn with_allowed_numbers(mut self, numbers: Vec<String>) -> Self {
        self.allowed_numbers = numbers;
        self
    }

    /// Get API URL for messages endpoint
    fn messages_url(&self) -> String {
        format!(
            "https://graph.facebook.com/{}/{}/messages",
            self.api_version, self.phone_number_id
        )
    }
}

/// WhatsApp Business API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // All fields needed for JSON deserialization
struct ApiResponse {
    messaging_product: Option<String>,
    contacts: Option<Vec<Contact>>,
    messages: Option<Vec<MessageInfo>>,
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // All fields needed for JSON deserialization
struct Contact {
    input: String,
    wa_id: String,
}

#[derive(Debug, Deserialize)]
struct MessageInfo {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
    code: i32,
}

/// Incoming webhook event from WhatsApp Business API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppBusinessWebhook {
    /// Object type (should be "whatsapp_business_account")
    pub object: String,
    /// Entry array
    pub entry: Vec<WebhookEntry>,
}

/// Webhook entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEntry {
    /// Business Account ID
    pub id: String,
    /// Changes array
    pub changes: Vec<WebhookChange>,
}

/// Webhook change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookChange {
    /// Value containing the actual message data
    pub value: WebhookValue,
    /// Field name
    pub field: String,
}

/// Webhook value containing message data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookValue {
    /// Messaging product
    pub messaging_product: String,
    /// Metadata
    pub metadata: WebhookMetadata,
    /// Contacts (sender info)
    #[serde(default)]
    pub contacts: Vec<WebhookContact>,
    /// Messages
    #[serde(default)]
    pub messages: Vec<WebhookMessage>,
    /// Statuses (delivery receipts)
    #[serde(default)]
    pub statuses: Vec<WebhookStatus>,
}

/// Webhook metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookMetadata {
    /// Display phone number
    pub display_phone_number: String,
    /// Phone number ID
    pub phone_number_id: String,
}

/// Webhook contact (sender info)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookContact {
    /// Profile info
    pub profile: Option<WebhookProfile>,
    /// Phone number
    pub wa_id: String,
}

/// Webhook profile (user profile)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookProfile {
    /// Display name
    pub name: String,
}

/// Webhook message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookMessage {
    /// Sender phone number
    pub from: String,
    /// Message ID
    pub id: String,
    /// Timestamp
    pub timestamp: String,
    /// Message type
    #[serde(rename = "type")]
    pub message_type: String,
    /// Text content (for text messages)
    pub text: Option<TextContent>,
}

/// Text content in message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContent {
    /// Message body
    pub body: String,
}

/// Webhook status (delivery receipts)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookStatus {
    /// Message ID
    pub id: String,
    /// Status (sent, delivered, read)
    pub status: String,
    /// Timestamp
    pub timestamp: String,
    /// Recipient ID
    pub recipient_id: String,
}

/// WhatsApp Business API adapter
pub struct WhatsAppBusinessAdapter {
    config: WhatsAppBusinessConfig,
    client: reqwest::Client,
}

impl WhatsAppBusinessAdapter {
    /// Create a new WhatsApp Business adapter
    #[must_use]
    pub fn new(config: WhatsAppBusinessConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        info!("WhatsApp Business API adapter initialized");
        info!("Phone Number ID: {}", config.phone_number_id);

        Self { config, client }
    }

    /// Create from environment
    pub fn from_env() -> Result<Self> {
        let config = WhatsAppBusinessConfig::from_env()?;
        Ok(Self::new(config))
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

    /// Handle incoming webhook
    #[instrument(skip(self, orchestrator, webhook))]
    pub async fn handle_webhook(
        &self,
        orchestrator: Arc<Orchestrator>,
        webhook: WhatsAppBusinessWebhook,
    ) -> Result<()> {
        let messages = self.extract_messages(&webhook);

        for (sender_name, msg) in messages {
            let Some(normalized) = self.normalize_webhook_message(&sender_name, &msg) else {
                continue;
            };

            info!(
                from = %normalized.user_id,
                text = %mask_for_logging(&normalized.text),
                "Received WhatsApp Business message"
            );

            // Process with orchestrator
            let input = OrchestratorInput::new(
                "whatsapp",
                &normalized.channel_id,
                &normalized.user_id,
                &normalized.text,
            );

            match orchestrator.process(input).await {
                Ok(result) => {
                    let response_text = if result.response.is_empty() {
                        "Done.".to_string()
                    } else {
                        result.response
                    };

                    // WhatsApp Business API has a 4096 character limit for text messages
                    if response_text.len() > 4096 {
                        for chunk in response_text.as_bytes().chunks(4096) {
                            if let Ok(text) = std::str::from_utf8(chunk) {
                                let _ = self
                                    .send_message(&msg.from, OutgoingMessage::text(text))
                                    .await;
                            }
                        }
                    } else {
                        let _ = self
                            .send_message(&msg.from, OutgoingMessage::text(response_text))
                            .await;
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to process WhatsApp Business message");
                    let _ = self
                        .send_message(
                            &msg.from,
                            OutgoingMessage::text(
                                "Sorry, I encountered an error. Please try again.",
                            ),
                        )
                        .await;
                }
            }
        }

        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        let config = WhatsAppBusinessConfig::new("token", "phone_id", "business_id")
            .with_webhook_verify_token("my_token")
            .with_allowed_numbers(vec!["+821012345678".to_string()]);

        assert_eq!(config.access_token, "token");
        assert_eq!(config.phone_number_id, "phone_id");
        assert_eq!(config.webhook_verify_token, "my_token");
        assert_eq!(config.allowed_numbers.len(), 1);
    }

    #[test]
    fn test_number_allowed() {
        let config = WhatsAppBusinessConfig::new("token", "phone_id", "business_id")
            .with_allowed_numbers(vec!["+821012345678".to_string()]);
        let adapter = WhatsAppBusinessAdapter::new(config);

        assert!(adapter.is_number_allowed("+821012345678"));
        assert!(adapter.is_number_allowed("821012345678"));
        assert!(!adapter.is_number_allowed("+821099999999"));
    }

    #[test]
    fn test_empty_allowlist_allows_all() {
        let config = WhatsAppBusinessConfig::new("token", "phone_id", "business_id");
        let adapter = WhatsAppBusinessAdapter::new(config);

        assert!(adapter.is_number_allowed("+821012345678"));
        assert!(adapter.is_number_allowed("+14155551234"));
    }

    #[test]
    fn test_verify_webhook() {
        let config = WhatsAppBusinessConfig::new("token", "phone_id", "business_id")
            .with_webhook_verify_token("my_verify_token");
        let adapter = WhatsAppBusinessAdapter::new(config);

        // Valid verification
        let result = adapter.verify_webhook("subscribe", "my_verify_token", "challenge_123");
        assert_eq!(result, Some("challenge_123".to_string()));

        // Invalid token
        let result = adapter.verify_webhook("subscribe", "wrong_token", "challenge_123");
        assert_eq!(result, None);

        // Invalid mode
        let result = adapter.verify_webhook("unsubscribe", "my_verify_token", "challenge_123");
        assert_eq!(result, None);
    }
}
