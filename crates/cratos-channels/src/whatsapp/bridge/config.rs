use crate::error::Result;
use serde::Deserialize;

/// Default Baileys bridge server URL
pub const DEFAULT_BRIDGE_URL: &str = "http://localhost:3001";

/// Default request timeout in seconds
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

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
