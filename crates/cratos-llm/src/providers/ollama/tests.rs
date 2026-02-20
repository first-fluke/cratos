use super::convert;
use super::security::sanitize_api_error;
use super::types::{OllamaConfig, DEFAULT_BASE_URL, DEFAULT_MODEL};
use crate::router::Message;
use std::time::Duration;

#[test]
fn test_config_builder() {
    let config = OllamaConfig::new()
        .with_model("mistral")
        .with_base_url("http://192.168.1.100:11434")
        .with_max_tokens(2048)
        .with_timeout(Duration::from_secs(120));

    assert_eq!(config.default_model, "mistral");
    assert_eq!(config.base_url, "http://192.168.1.100:11434");
    assert_eq!(config.default_max_tokens, 2048);
    assert_eq!(config.timeout, Duration::from_secs(120));
}

#[test]
fn test_default_config() {
    let config = OllamaConfig::default();

    assert_eq!(config.base_url, DEFAULT_BASE_URL);
    assert_eq!(config.default_model, DEFAULT_MODEL);
    assert_eq!(config.timeout, Duration::from_secs(300));
}

#[test]
fn test_message_conversion() {
    let messages = vec![
        Message::system("You are helpful"),
        Message::user("Hello"),
        Message::assistant("Hi there!"),
    ];

    let converted = convert::convert_messages(&messages);

    assert_eq!(converted.len(), 3);
    assert_eq!(converted[0].role, "system");
    assert_eq!(converted[1].role, "user");
    assert_eq!(converted[2].role, "assistant");
}

// Security tests

#[test]
fn test_sanitize_api_error() {
    // Path exposure should be sanitized
    let sanitized = sanitize_api_error("Error loading model from /home/user/.ollama/models");
    assert!(!sanitized.contains("/home"));
    assert!(sanitized.contains("installation"));

    // Connection errors should give helpful message
    let sanitized = sanitize_api_error("connection refused");
    assert!(sanitized.contains("Ollama running"));

    // Model errors should suggest pull
    let sanitized = sanitize_api_error("model 'llama3' not found");
    assert!(sanitized.contains("pull"));
}
