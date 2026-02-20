use super::types::{AnthropicConfig, MODELS};
use super::convert::convert_messages;
use super::security::sanitize_api_error;
use crate::router::Message;
use crate::util::mask_api_key;
use std::time::Duration;

#[test]
fn test_config_builder() {
    let config = AnthropicConfig::new("test-key")
        .with_model("claude-3-haiku-20240307")
        .with_max_tokens(2048)
        .with_timeout(Duration::from_secs(30));

    assert_eq!(config.api_key, "test-key");
    assert_eq!(config.default_model, "claude-3-haiku-20240307");
    assert_eq!(config.default_max_tokens, 2048);
    assert_eq!(config.timeout, Duration::from_secs(30));
}

#[test]
fn test_available_models() {
    assert!(MODELS.contains(&"claude-sonnet-4-5-20250929"));
    assert!(MODELS.contains(&"claude-opus-4-5-20250514"));
    assert!(MODELS.contains(&"claude-haiku-4-5-20251001"));
    assert!(MODELS.contains(&"claude-3-5-sonnet-20241022"));
}

#[test]
fn test_message_conversion() {
    let messages = vec![
        Message::system("You are helpful"),
        Message::user("Hello"),
        Message::assistant("Hi there!"),
    ];

    let (system, converted) = convert_messages(&messages);

    assert_eq!(system, Some("You are helpful".to_string()));
    assert_eq!(converted.len(), 2);
    assert_eq!(converted[0].role, "user");
    assert_eq!(converted[1].role, "assistant");
}

// Security tests

#[test]
fn test_api_key_masking() {
    let masked = mask_api_key("sk-ant-1234567890abcdefghij");
    assert!(masked.starts_with("sk-a"));
    assert!(masked.contains("..."));
    assert!(!masked.contains("1234567890"));
}

#[test]
fn test_sanitize_api_error() {
    let sanitized = sanitize_api_error("Invalid x-api-key header");
    assert!(!sanitized.contains("x-api-key"));
    assert!(sanitized.contains("authentication"));

    let sanitized = sanitize_api_error("overloaded: too many requests");
    assert!(sanitized.contains("rate limit"));
}

#[test]
fn test_config_debug_masks_key() {
    let config = AnthropicConfig::new("sk-ant-1234567890abcdefghij");
    let debug_str = format!("{:?}", config);

    assert!(!debug_str.contains("1234567890"));
    assert!(debug_str.contains("sk-a...ghij"));
}
