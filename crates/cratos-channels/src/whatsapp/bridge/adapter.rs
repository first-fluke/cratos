use super::config::WhatsAppConfig;
use super::types::{
    ConnectionStatus, SendResponse, StatusResponse, WhatsAppConnection, WhatsAppWebhookMessage,
};
use crate::error::{Error, Result};
use crate::message::{
    ChannelAdapter, ChannelType, NormalizedMessage, OutgoingAttachment, OutgoingMessage,
};

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

use tracing::{debug, info, warn};

/// WhatsApp adapter (Baileys bridge)
pub struct WhatsAppAdapter {
    config: WhatsAppConfig,
    client: reqwest::Client,
    connected: AtomicBool,
}

impl WhatsAppAdapter {
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

    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let url = format!("{}/delete", self.config.bridge_url);

        #[derive(Serialize)]
        struct DeleteRequest<'a> {
            to: &'a str,
            #[serde(rename = "messageId")]
            message_id: &'a str,
        }

        let resp: SendResponse = self
            .client
            .post(&url)
            .json(&DeleteRequest {
                to: channel_id,
                message_id,
            })
            .send()
            .await
            .map_err(|e| Error::Bridge(format!("Failed to delete message: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::WhatsApp(format!("Invalid delete response: {}", e)))?;

        if resp.success {
            Ok(())
        } else {
            Err(Error::WhatsApp(
                resp.error.unwrap_or_else(|| "Delete failed".to_string()),
            ))
        }
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

    async fn send_attachment(
        &self,
        channel_id: &str,
        attachment: OutgoingAttachment,
        reply_to: Option<&str>,
    ) -> Result<String> {
        use base64::Engine as _;

        // Decode base64 attachment data
        let file_data = base64::engine::general_purpose::STANDARD
            .decode(&attachment.data)
            .map_err(|e| Error::WhatsApp(format!("Invalid base64 attachment data: {}", e)))?;

        // Determine media type for WhatsApp
        let media_type = match attachment.mime_type.split('/').next().unwrap_or("document") {
            "image" => "image",
            "video" => "video",
            "audio" => "audio",
            _ => "document",
        };

        // Upload media to WhatsApp bridge
        let upload_url = format!("{}/media/upload", self.config.bridge_url);

        #[derive(Serialize)]
        struct UploadRequest<'a> {
            media_type: &'a str,
            filename: &'a str,
            mime_type: &'a str,
            data: String, // base64
        }

        #[derive(Deserialize)]
        struct UploadResponse {
            media_id: Option<String>,
            error: Option<String>,
        }

        let upload_resp: UploadResponse = self
            .client
            .post(&upload_url)
            .json(&UploadRequest {
                media_type,
                filename: &attachment.filename,
                mime_type: &attachment.mime_type,
                data: base64::engine::general_purpose::STANDARD.encode(&file_data),
            })
            .send()
            .await
            .map_err(|e| Error::Bridge(format!("Failed to upload media: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::Bridge(format!("Failed to parse upload response: {}", e)))?;

        let media_id = upload_resp.media_id.ok_or_else(|| {
            Error::WhatsApp(
                upload_resp
                    .error
                    .unwrap_or_else(|| "Upload failed".to_string()),
            )
        })?;

        // Send media message
        let send_url = format!("{}/message/media", self.config.bridge_url);

        #[derive(Serialize)]
        struct MediaMessageRequest<'a> {
            to: &'a str,
            media_type: &'a str,
            media_id: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            caption: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            reply_to: Option<&'a str>,
        }

        #[derive(Deserialize)]
        struct MediaMessageResponse {
            message_id: Option<String>,
            error: Option<String>,
        }

        let resp: MediaMessageResponse = self
            .client
            .post(&send_url)
            .json(&MediaMessageRequest {
                to: channel_id,
                media_type,
                media_id: &media_id,
                caption: attachment.caption.as_deref(),
                reply_to,
            })
            .send()
            .await
            .map_err(|e| Error::Bridge(format!("Failed to send media message: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::Bridge(format!("Failed to parse response: {}", e)))?;

        resp.message_id.ok_or_else(|| {
            Error::WhatsApp(
                resp.error
                    .unwrap_or_else(|| "Send media failed".to_string()),
            )
        })
    }
}
