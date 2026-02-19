use serde::{Deserialize, Serialize};

/// Connection status response
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // All fields needed for JSON deserialization
pub struct StatusResponse {
    pub status: String,
    pub qr: Option<String>,
    pub connected: bool,
}

/// Send message response
#[derive(Debug, Deserialize)]
pub struct SendResponse {
    pub success: bool,
    #[serde(rename = "messageId")]
    pub message_id: Option<String>,
    pub error: Option<String>,
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
