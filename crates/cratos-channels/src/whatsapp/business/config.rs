use crate::error::{Error, Result};
use serde::Deserialize;

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
    pub(crate) fn messages_url(&self) -> String {
        format!(
            "https://graph.facebook.com/{}/{}/messages",
            self.api_version, self.phone_number_id
        )
    }
}
