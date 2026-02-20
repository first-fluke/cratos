#[cfg(test)]

    use super::*;
    use crate::util::mask_api_key;
    use std::time::Duration;

    #[test]
    fn test_config_builder() {
        let config = DeepSeekConfig::new("test-key")
            .with_model("deepseek-coder")
            .with_timeout(Duration::from_secs(90));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "deepseek-coder");
        assert_eq!(config.timeout, Duration::from_secs(90));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"deepseek-chat"));
        assert!(MODELS.contains(&"deepseek-coder"));
    }

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("sk-1234567890abcdefghijklmnop");
        assert!(masked.starts_with("sk-1"));
        assert!(masked.ends_with("mnop"));
        assert!(masked.contains("..."));
    }

    #[test]
    fn test_sanitize_api_error() {
        let sanitized = sanitize_api_error("Invalid API key: sk-1234567890");
        assert!(!sanitized.contains("sk-"));
        assert!(sanitized.contains("DEEPSEEK_API_KEY"));

        let sanitized = sanitize_api_error("Rate limit exceeded");
        assert!(sanitized.contains("rate limit"));
    }

    #[test]
    fn test_config_debug_masks_key() {
        let config = DeepSeekConfig::new("sk-1234567890abcdefghijklmnop");
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.contains("1234567890abcdefghijkl"));
    }

