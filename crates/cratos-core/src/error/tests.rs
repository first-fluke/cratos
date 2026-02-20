
    use super::*;

    #[test]
    fn test_api_key_missing_message() {
        let error = Error::ApiKeyMissing {
            provider: "Anthropic".to_string(),
        };

        let msg = error.user_message();
        assert!(msg.contains("Anthropic"));
        assert!(msg.contains("API key"));

        let suggestion = error.suggestion().unwrap();
        assert!(suggestion.contains("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn test_rate_limited_message() {
        let error = Error::RateLimited {
            retry_after: Some(30),
        };

        let msg = error.user_message();
        assert!(msg.contains("30 seconds"));

        let suggestion = error.suggestion().unwrap();
        assert!(suggestion.contains("different model"));
    }

    #[test]
    fn test_network_error_message() {
        let error = Error::NetworkError("connection refused".to_string());

        let msg = error.user_message();
        assert!(msg.contains("Network"));

        let url = error.docs_url().unwrap();
        assert!(url.contains("network"));
    }

    #[test]
    fn test_invalid_config_message() {
        let error = Error::InvalidConfig {
            field: "llm.timeout".to_string(),
            message: "must be positive".to_string(),
        };

        let msg = error.user_message();
        assert!(msg.contains("llm.timeout"));
        assert!(msg.contains("must be positive"));

        let suggestion = error.suggestion().unwrap();
        assert!(suggestion.contains("llm.timeout"));
    }

    #[test]
    fn test_format_error_for_cli() {
        let error = Error::ApiKeyMissing {
            provider: "OpenAI".to_string(),
        };

        let output = format_error_for_cli(&error);
        assert!(output.contains("OpenAI"));
        assert!(output.contains("OPENAI_API_KEY"));
        assert!(output.contains("docs.cratos.dev"));
    }

    #[test]
    fn test_format_error_for_chat() {
        let error = Error::NetworkError("timeout".to_string());

        let output = format_error_for_chat(&error);
        assert!(output.contains("Network"));
        assert!(output.contains("internet connection"));
    }

    #[test]
    fn test_memory_error_message() {
        let error = Error::Memory(MemoryStoreError::Cache("failed to fetch".to_string()));
        let msg = error.user_message();
        assert!(msg.contains("Memory error"));
        assert!(msg.contains("failed to fetch"));
    }

    #[test]
    fn test_chronicle_not_found_message() {
        let error = Error::Chronicle(ChronicleError::NotFound("abc-123".to_string()));
        let msg = error.user_message();
        assert!(msg.contains("Chronicle error"));
        assert!(msg.contains("not found: abc-123"));
    }
