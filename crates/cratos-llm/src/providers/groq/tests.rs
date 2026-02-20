    use super::*;
    use crate::util::mask_api_key;
    use std::time::Duration;

    #[test]
    fn test_config_builder() {
        let config = GroqConfig::new("test-key")
            .with_model("llama-3.1-8b-instant")
            .with_timeout(Duration::from_secs(30));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "llama-3.1-8b-instant");
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"llama-3.3-70b-versatile"));
        assert!(MODELS.contains(&"openai/gpt-oss-20b"));
        assert!(MODELS.contains(&"openai/gpt-oss-120b"));
    }

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("gsk_1234567890abcdefghijklmnop");
        assert!(masked.starts_with("gsk_"));
        assert!(masked.ends_with("mnop"));
        assert!(masked.contains("..."));
    }

    #[test]
    fn test_sanitize_api_error() {
        let sanitized = sanitize_api_error("Invalid API key: gsk_1234567890");
        assert!(!sanitized.contains("gsk_"));
        assert!(sanitized.contains("GROQ_API_KEY"));

        let sanitized = sanitize_api_error("Rate limit exceeded");
        assert!(sanitized.contains("rate limit"));
        assert!(sanitized.contains("30 req/min"));
    }

    #[test]
    fn test_config_debug_masks_key() {
        let config = GroqConfig::new("gsk_1234567890abcdefghijklmnop");
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.contains("1234567890abcdefghijkl"));
    }

    #[test]
    fn test_model_supports_tools() {
        assert!(GroqProvider::model_supports_tools("openai/gpt-oss-20b"));
        assert!(GroqProvider::model_supports_tools("openai/gpt-oss-120b"));
        assert!(!GroqProvider::model_supports_tools(
            "llama-3.3-70b-versatile"
        ));
        assert!(!GroqProvider::model_supports_tools("llama-3.1-8b-instant"));
        assert!(!GroqProvider::model_supports_tools("qwen/qwen3-32b"));
    }
