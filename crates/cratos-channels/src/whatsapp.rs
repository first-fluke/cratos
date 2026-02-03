//! WhatsApp - Baileys bridge adapter
//!
//! This module provides the WhatsApp adapter using a Node.js Baileys bridge.
//!
//! # Warning
//!
//! Baileys is an **unofficial** reverse-engineered WhatsApp Web library.
//! Using it carries these risks:
//! - **Account ban** - WhatsApp may permanently ban your number
//! - **ToS violation** - This violates Meta's Terms of Service
//! - **Instability** - May break with WhatsApp updates
//!
//! For production/business use, consider the official WhatsApp Business API instead.

use crate::error::{Error, Result};
use crate::message::{ChannelAdapter, ChannelType, NormalizedMessage, OutgoingMessage};
use cratos_core::{Orchestrator, OrchestratorInput};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

/// Maximum length of text to log
const MAX_LOG_TEXT_LENGTH: usize = 50;

/// Default Baileys bridge server URL
const DEFAULT_BRIDGE_URL: &str = "http://localhost:3001";

/// Default request timeout in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 30;

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

/// WhatsApp (Baileys) configuration
#[derive(Debug, Clone, Deserialize)]
pub struct WhatsAppConfig {
    /// Bridge server URL (default: http://localhost:3001)
    #[serde(default = "default_bridge_url")]
    pub bridge_url: String,
    /// Allowed phone numbers (empty = allow all)
    #[serde(default)]
    pub allowed_numbers: Vec<String>,
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_bridge_url() -> String {
    DEFAULT_BRIDGE_URL.to_string()
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

impl Default for WhatsAppConfig {
    fn default() -> Self {
        Self {
            bridge_url: default_bridge_url(),
            allowed_numbers: Vec::new(),
            timeout_secs: default_timeout(),
        }
    }
}

impl WhatsAppConfig {
    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let bridge_url =
            std::env::var("WHATSAPP_BRIDGE_URL").unwrap_or_else(|_| default_bridge_url());

        let allowed_numbers: Vec<String> = std::env::var("WHATSAPP_ALLOWED_NUMBERS")
            .ok()
            .map(|s| s.split(',').map(|n| n.trim().to_string()).collect())
            .unwrap_or_default();

        let timeout_secs = std::env::var("WHATSAPP_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(default_timeout());

        Ok(Self {
            bridge_url,
            allowed_numbers,
            timeout_secs,
        })
    }

    /// Create with bridge URL
    #[must_use]
    pub fn new(bridge_url: impl Into<String>) -> Self {
        Self {
            bridge_url: bridge_url.into(),
            ..Default::default()
        }
    }

    /// Set allowed numbers
    #[must_use]
    pub fn with_allowed_numbers(mut self, numbers: Vec<String>) -> Self {
        self.allowed_numbers = numbers;
        self
    }
}

/// Connection status response
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // All fields needed for JSON deserialization
struct StatusResponse {
    status: String,
    qr: Option<String>,
    connected: bool,
}

/// Send message response
#[derive(Debug, Deserialize)]
struct SendResponse {
    success: bool,
    #[serde(rename = "messageId")]
    message_id: Option<String>,
    error: Option<String>,
}

/// Incoming webhook message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppWebhookMessage {
    /// Message ID
    pub id: String,
    /// Sender JID (phone@s.whatsapp.net or group@g.us)
    pub from: String,
    /// Participant (for group messages)
    pub participant: Option<String>,
    /// Message text
    pub text: String,
    /// Unix timestamp
    pub timestamp: i64,
    /// Is group message
    #[serde(rename = "isGroup")]
    pub is_group: bool,
}

/// WhatsApp connection status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Not connected
    Disconnected,
    /// Waiting for QR scan
    WaitingScan,
    /// Connected and ready
    Connected,
}

/// WhatsApp adapter (Baileys bridge)
pub struct WhatsAppAdapter {
    config: WhatsAppConfig,
    client: reqwest::Client,
    connected: AtomicBool,
}

impl WhatsAppAdapter {
    /// Create a new WhatsApp adapter
    ///
    /// # Warning
    ///
    /// This displays a warning about the risks of using unofficial APIs.
    /// Create a new WhatsApp adapter
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(config: WhatsAppConfig) -> Result<Self> {
        // Display warning about risks
        warn!("========================================");
        warn!("   WHATSAPP (BAILEYS) - IMPORTANT WARNING");
        warn!("========================================");
        warn!("Using unofficial reverse-engineered API:");
        warn!("  - Account BAN risk exists");
        warn!("  - Violates Meta Terms of Service");
        warn!("  - Do NOT use with important accounts");
        warn!("  - For business: use WhatsApp Business API");
        warn!("========================================");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| Error::Network(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            config,
            client,
            connected: AtomicBool::new(false),
        })
    }

    /// Create from environment
    pub fn from_env() -> Result<Self> {
        let config = WhatsAppConfig::from_env()?;
        Self::new(config)
    }

    /// Check if a phone number is allowed
    pub fn is_number_allowed(&self, number: &str) -> bool {
        if self.config.allowed_numbers.is_empty() {
            return true;
        }
        // Normalize number for comparison
        let normalized = number
            .replace("@s.whatsapp.net", "")
            .replace("@g.us", "")
            .replace("+", "")
            .replace("-", "")
            .replace(" ", "");

        self.config.allowed_numbers.iter().any(|allowed| {
            let norm_allowed = allowed.replace("+", "").replace("-", "").replace(" ", "");
            normalized.contains(&norm_allowed) || norm_allowed.contains(&normalized)
        })
    }

    /// Check connection status
    pub async fn status(&self) -> Result<ConnectionStatus> {
        let url = format!("{}/status", self.config.bridge_url);

        let resp: StatusResponse = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Bridge(format!("Failed to check status: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::Bridge(format!("Invalid status response: {}", e)))?;

        let status = match resp.status.as_str() {
            "connected" => ConnectionStatus::Connected,
            "waiting_scan" => ConnectionStatus::WaitingScan,
            _ => ConnectionStatus::Disconnected,
        };

        self.connected
            .store(status == ConnectionStatus::Connected, Ordering::SeqCst);

        Ok(status)
    }

    /// Start connection (returns QR code if needed)
    pub async fn connect(&self) -> Result<WhatsAppConnection> {
        let url = format!("{}/connect", self.config.bridge_url);

        let resp: StatusResponse = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| Error::Bridge(format!("Failed to connect: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::Bridge(format!("Invalid connect response: {}", e)))?;

        match resp.status.as_str() {
            "connected" => {
                self.connected.store(true, Ordering::SeqCst);
                info!("WhatsApp connected");
                Ok(WhatsAppConnection::Connected)
            }
            "waiting_scan" => {
                if let Some(qr) = resp.qr {
                    info!("WhatsApp QR code generated - scan with your phone");
                    Ok(WhatsAppConnection::QrCode(qr))
                } else {
                    Ok(WhatsAppConnection::WaitingScan)
                }
            }
            _ => Err(Error::WhatsApp("Unknown connection status".to_string())),
        }
    }

    /// Disconnect from WhatsApp
    pub async fn disconnect(&self) -> Result<()> {
        let url = format!("{}/disconnect", self.config.bridge_url);

        self.client
            .post(&url)
            .send()
            .await
            .map_err(|e| Error::Bridge(format!("Failed to disconnect: {}", e)))?;

        self.connected.store(false, Ordering::SeqCst);
        info!("WhatsApp disconnected");

        Ok(())
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Convert webhook message to normalized message
    pub fn normalize_webhook_message(
        &self,
        msg: &WhatsAppWebhookMessage,
    ) -> Option<NormalizedMessage> {
        // Check if number is allowed
        let sender = msg.participant.as_ref().unwrap_or(&msg.from);
        if !self.is_number_allowed(sender) {
            debug!(from = %sender, "Number not allowed");
            return None;
        }

        // Extract user ID from JID
        let user_id = sender.split('@').next().unwrap_or(sender).to_string();
        let channel_id = msg.from.clone();

        let normalized = NormalizedMessage::new(
            ChannelType::WhatsApp,
            channel_id,
            &user_id,
            &msg.id,
            &msg.text,
        );

        Some(normalized)
    }

    /// Handle incoming webhook message
    #[instrument(skip(self, orchestrator, msg))]
    pub async fn handle_webhook(
        &self,
        orchestrator: Arc<Orchestrator>,
        msg: WhatsAppWebhookMessage,
    ) -> Result<()> {
        let Some(normalized) = self.normalize_webhook_message(&msg) else {
            return Ok(());
        };

        info!(
            from = %normalized.user_id,
            text = %mask_for_logging(&normalized.text),
            "Received WhatsApp message"
        );

        // Send typing indicator
        let _ = self.send_typing(&msg.from).await;

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

                // WhatsApp has no strict message length limit, but split long messages
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
                error!(error = %e, "Failed to process WhatsApp message");
                let _ = self
                    .send_message(
                        &msg.from,
                        OutgoingMessage::text("Sorry, I encountered an error. Please try again."),
                    )
                    .await;
            }
        }

        Ok(())
    }
}

/// WhatsApp connection result
#[derive(Debug)]
pub enum WhatsAppConnection {
    /// Already connected
    Connected,
    /// QR code to scan
    QrCode(String),
    /// Waiting for scan (no QR available yet)
    WaitingScan,
}

#[async_trait::async_trait]
impl ChannelAdapter for WhatsAppAdapter {
    fn channel_type(&self) -> ChannelType {
        ChannelType::WhatsApp
    }

    async fn send_message(&self, channel_id: &str, message: OutgoingMessage) -> Result<String> {
        let url = format!("{}/send", self.config.bridge_url);

        #[derive(Serialize)]
        struct SendRequest<'a> {
            to: &'a str,
            message: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            #[serde(rename = "quotedId")]
            quoted_id: Option<&'a str>,
        }

        let request = SendRequest {
            to: channel_id,
            message: &message.text,
            quoted_id: message.reply_to.as_deref(),
        };

        let resp: SendResponse = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Bridge(format!("Failed to send message: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::WhatsApp(format!("Invalid send response: {}", e)))?;

        if resp.success {
            Ok(resp.message_id.unwrap_or_default())
        } else {
            Err(Error::WhatsApp(
                resp.error.unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    async fn edit_message(
        &self,
        _channel_id: &str,
        _message_id: &str,
        _message: OutgoingMessage,
    ) -> Result<()> {
        // WhatsApp doesn't support editing messages
        Err(Error::NotEnabled(
            "WhatsApp doesn't support message editing".to_string(),
        ))
    }

    async fn delete_message(&self, _channel_id: &str, _message_id: &str) -> Result<()> {
        // WhatsApp message deletion is complex and requires the original message
        Err(Error::NotEnabled(
            "WhatsApp message deletion not implemented".to_string(),
        ))
    }

    async fn send_typing(&self, channel_id: &str) -> Result<()> {
        let url = format!("{}/typing", self.config.bridge_url);

        #[derive(Serialize)]
        struct TypingRequest<'a> {
            to: &'a str,
        }

        self.client
            .post(&url)
            .json(&TypingRequest { to: channel_id })
            .send()
            .await
            .map_err(|e| Error::Bridge(format!("Failed to send typing: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whatsapp_config() {
        let config = WhatsAppConfig::new("http://localhost:3001")
            .with_allowed_numbers(vec!["+821012345678".to_string()]);

        assert_eq!(config.bridge_url, "http://localhost:3001");
        assert_eq!(config.allowed_numbers.len(), 1);
    }

    #[test]
    fn test_number_allowed() {
        let config =
            WhatsAppConfig::default().with_allowed_numbers(vec!["+821012345678".to_string()]);
        let adapter = WhatsAppAdapter::new(config);

        assert!(adapter.is_number_allowed("+821012345678@s.whatsapp.net"));
        assert!(adapter.is_number_allowed("821012345678"));
        assert!(!adapter.is_number_allowed("+821099999999"));
    }

    #[test]
    fn test_empty_allowlist_allows_all() {
        let config = WhatsAppConfig::default();
        let adapter = WhatsAppAdapter::new(config);

        assert!(adapter.is_number_allowed("+821012345678"));
        assert!(adapter.is_number_allowed("+14155551234"));
    }

    #[test]
    fn test_mask_for_logging() {
        assert!(mask_for_logging("my password is test").contains("REDACTED"));
        assert_eq!(mask_for_logging("Hello"), "Hello");

        let long_text = "a".repeat(100);
        let masked = mask_for_logging(&long_text);
        assert!(masked.contains("..."));
    }
}
