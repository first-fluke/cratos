use serde::{Deserialize, Serialize};

/// WhatsApp Business API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // All fields needed for JSON deserialization
pub struct ApiResponse {
    pub messaging_product: Option<String>,
    pub contacts: Option<Vec<Contact>>,
    pub messages: Option<Vec<MessageInfo>>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // All fields needed for JSON deserialization
pub struct Contact {
    pub input: String,
    pub wa_id: String,
}

#[derive(Debug, Deserialize)]
pub struct MessageInfo {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub message: String,
    pub code: i32,
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
