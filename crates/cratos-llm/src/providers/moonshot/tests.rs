#[cfg(test)]

    use super::*;
    use crate::util::mask_api_key;
    use std::time::Duration;

    #[test]
    fn test_config_builder() {
        let config = MoonshotConfig::new("test-key")
            .with_model("kimi-k2")
            .with_timeout(Duration::from_secs(60));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "kimi-k2");
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"kimi-k2-5"));
        assert!(MODELS.contains(&"kimi-k2"));
    }

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("ms_1234567890abcdefghijklmnop");
        assert!(masked.starts_with("ms_"));
        assert!(masked.ends_with("mnop"));
        assert!(masked.contains("..."));
    }

    #[test]
    fn test_sanitize_api_error() {
        let sanitized = sanitize_api_error("Invalid API key: ms_1234567890");
        assert!(!sanitized.contains("ms_"));
        assert!(sanitized.contains("MOONSHOT_API_KEY"));

        let sanitized = sanitize_api_error("Rate limit exceeded");
        assert!(sanitized.contains("rate limit"));
    }

    #[test]
    fn test_config_debug_masks_key() {
        let config = MoonshotConfig::new("ms_1234567890abcdefghijklmnop");
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.contains("1234567890abcdefghijkl"));
    }

