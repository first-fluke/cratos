//! WhatsApp Business API adapter

/// WhatsApp Business Cloud API adapter.
pub mod adapter;
/// WhatsApp Business API configuration and credentials.
pub mod config;
/// Incoming webhook handler for WhatsApp Business API.
pub mod handler;
/// WhatsApp Business API request/response types.
pub mod types;

pub use adapter::WhatsAppBusinessAdapter;
pub use config::WhatsAppBusinessConfig;
pub use handler::WhatsAppBusinessHandler;
pub use types::*;

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
        let adapter = WhatsAppBusinessAdapter::new(config).expect("Failed to create adapter");

        assert!(adapter.is_number_allowed("+821012345678"));
        assert!(adapter.is_number_allowed("821012345678"));
        assert!(!adapter.is_number_allowed("+821099999999"));
    }

    #[test]
    fn test_empty_allowlist_allows_all() {
        let config = WhatsAppBusinessConfig::new("token", "phone_id", "business_id");
        let adapter = WhatsAppBusinessAdapter::new(config).expect("Failed to create adapter");

        assert!(adapter.is_number_allowed("+821012345678"));
        assert!(adapter.is_number_allowed("+14155551234"));
    }

    #[test]
    fn test_verify_webhook() {
        let config = WhatsAppBusinessConfig::new("token", "phone_id", "business_id")
            .with_webhook_verify_token("my_verify_token");
        let adapter = WhatsAppBusinessAdapter::new(config).expect("Failed to create adapter");

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
