#[cfg(test)]

    use super::*;
    use crate::util::mask_api_key;
    use std::time::Duration;

    #[test]
    fn test_config_builder() {
        let config = QwenConfig::new("test-key")
            .with_model("qwen-plus")
            .with_timeout(Duration::from_secs(30));

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.default_model, "qwen-plus");
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_available_models() {
        assert!(MODELS.contains(&"qwen-turbo"));
        assert!(MODELS.contains(&"qwen-plus"));
        assert!(MODELS.contains(&"qwen-max"));
    }

    #[test]
    fn test_api_key_masking() {
        let masked = mask_api_key("sk-1234567890abcdefghij");
        assert!(masked.starts_with("sk-1"));
        assert!(masked.ends_with("ghij"));
    }

    #[test]
    fn test_convert_message() {
        let msg = Message::system("You are helpful");
        let converted = QwenProvider::convert_message(&msg);
        assert_eq!(converted.role, "system");
        assert_eq!(converted.content, "You are helpful");
    }

    #[test]
    fn test_convert_tool_choice() {
        let auto = QwenProvider::convert_tool_choice(&ToolChoice::Auto);
        assert_eq!(auto, Some(serde_json::json!("auto")));

        let tool = QwenProvider::convert_tool_choice(&ToolChoice::Tool("my_tool".to_string()));
        assert!(tool.is_some());
        let tool_val = tool.unwrap();
        assert_eq!(tool_val["function"]["name"], "my_tool");
    }

