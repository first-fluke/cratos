use super::provider::OpenRouterProvider;
use super::types::{OpenRouterConfig, MODELS};
use crate::router::Message;
use crate::util::mask_api_key;
use std::time::Duration;

#[test]
fn test_config_builder() {
    let config = OpenRouterConfig::new("test-key")
        .with_model("openai/gpt-4o")
        .with_timeout(Duration::from_secs(60))
        .with_app_name("TestApp");

    assert_eq!(config.api_key, "test-key");
    assert_eq!(config.default_model, "openai/gpt-4o");
    assert_eq!(config.app_name, Some("TestApp".to_string()));
}

#[test]
fn test_available_models() {
    assert!(MODELS.contains(&"qwen/qwen3-32b:free"));
    assert!(MODELS.contains(&"openai/gpt-4o"));
    assert!(MODELS.contains(&"anthropic/claude-3.5-sonnet"));
}

#[test]
fn test_is_free_model() {
    assert!(OpenRouterProvider::is_free_model("qwen/qwen3-32b:free"));
    assert!(OpenRouterProvider::is_free_model(
        "meta-llama/llama-3.2-3b-instruct:free"
    ));
    assert!(!OpenRouterProvider::is_free_model("openai/gpt-4o"));
}

#[test]
fn test_api_key_masking() {
    let masked = mask_api_key("sk-or-1234567890abcdefghij");
    assert!(masked.starts_with("sk-o"));
    assert!(masked.ends_with("ghij"));
}

#[test]
fn test_convert_message() {
    let msg = Message::assistant("Hello!");
    let converted = OpenRouterProvider::convert_message(&msg);
    assert_eq!(converted.role, "assistant");
    assert_eq!(converted.content, "Hello!");
}
