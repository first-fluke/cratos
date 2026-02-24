//! WhatsApp - Baileys bridge adapter

/// WhatsApp adapter using Baileys Node.js bridge.
pub mod adapter;
/// WhatsApp bridge connection configuration.
pub mod config;
/// Incoming message handler for WhatsApp bridge webhooks.
pub mod handler;
/// Bridge API request/response types.
pub mod types;

pub use adapter::WhatsAppAdapter;
pub use config::WhatsAppConfig;
pub use handler::WhatsAppHandler;
pub use types::*;

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
        let adapter = WhatsAppAdapter::new(config).expect("Failed to create adapter");

        assert!(adapter.is_number_allowed("+821012345678@s.whatsapp.net"));
        assert!(adapter.is_number_allowed("821012345678"));
        assert!(!adapter.is_number_allowed("+821099999999"));
    }

    #[test]
    fn test_empty_allowlist_allows_all() {
        let config = WhatsAppConfig::default();
        let adapter = WhatsAppAdapter::new(config).expect("Failed to create adapter");

        assert!(adapter.is_number_allowed("+821012345678"));
        assert!(adapter.is_number_allowed("+14155551234"));
    }

    #[test]
    fn test_media_type_detection() {
        // Test media type classification used in send_attachment
        let test_cases = [
            ("image/jpeg", "image"),
            ("image/png", "image"),
            ("video/mp4", "video"),
            ("audio/mpeg", "audio"),
            ("application/pdf", "document"),
            ("text/plain", "document"),
        ];

        for (mime_type, expected) in test_cases {
            let detected = match mime_type.split('/').next().unwrap_or("document") {
                "image" => "image",
                "video" => "video",
                "audio" => "audio",
                _ => "document",
            };
            assert_eq!(
                detected, expected,
                "MIME type {} should map to {}",
                mime_type, expected
            );
        }
    }

    #[test]
    fn test_whatsapp_bridge_url_format() {
        let config = WhatsAppConfig::new("http://localhost:3001");

        let upload_url = format!("{}/media/upload", config.bridge_url);
        let send_url = format!("{}/message/media", config.bridge_url);

        assert_eq!(upload_url, "http://localhost:3001/media/upload");
        assert_eq!(send_url, "http://localhost:3001/message/media");
    }

    #[test]
    fn test_base64_encoding_for_upload() {
        use base64::Engine as _;

        let data = b"test file content";
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);

        // Verify it's valid base64
        assert!(base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .is_ok());
        assert!(!encoded.contains(' '));
    }
}
