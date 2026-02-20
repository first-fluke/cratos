    use super::*;
    use crate::util::mask_api_key;
    use crate::router::{Message, ToolChoice};
    use std::time::Duration;

    #[test]
    fn test_config_builder() {
        let config = GlmConfig::new("test-key")
            .with_model("glm-4-plus")
            .with_timeout(Duration::from_secs(30));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "glm-4-plus");
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"glm-4.7"));
        assert!(MODELS.contains(&"glm-4.7-flash"));
    }

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("1234567890abcdefghij");
        assert!(masked.starts_with("1234"));
        assert!(masked.ends_with("ghij"));
        assert!(masked.contains("..."));
    }

    #[test]
    fn test_config_debug_masks_key() {
        let config = GlmConfig::new("1234567890abcdefghij");
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.contains("567890abcdef"));
    }

    #[test]
    fn test_convert_message() {
        let msg = Message::user("Hello");
        let converted = GlmProvider::convert_message(msg);
        assert_eq!(converted.role, "user");
        assert_eq!(converted.content, "Hello");
    }

    #[test]
    fn test_convert_tool_choice() {
        assert_eq!(
            GlmProvider::convert_tool_choice(ToolChoice::Auto),
            Some("auto".to_string())
        );
        assert_eq!(
            GlmProvider::convert_tool_choice(ToolChoice::None),
            Some("none".to_string())
        );
    }
