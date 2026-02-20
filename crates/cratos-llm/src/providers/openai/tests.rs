use super::*;
use crate::util::mask_api_key;
use std::time::Duration;

#[test]
fn test_config_builder() {
    let config = OpenAiConfig::new("test-key")
        .with_model("gpt-4o-mini")
        .with_timeout(Duration::from_secs(30));

    assert_eq!(config.api_key, "test-key");
    assert_eq!(config.default_model, "gpt-4o-mini");
    assert_eq!(config.timeout, Duration::from_secs(30));
}

#[test]
fn test_available_models() {
    // MODELS might be private in mod.rs. If so, I need to make it pub(crate) or pub.
    // Assuming it's accessible via super::MODELS if pub(crate) or pub.
    // If it is private, I need to open it.
    // Let's assume pub(crate) for now or check visibility.
    // The previous view_file didn't show definition of MODELS.
    // But it's used here.
    assert!(MODELS.contains(&"gpt-4o"));
    assert!(MODELS.contains(&"gpt-4o-mini"));
}

#[test]
fn test_api_key_masking() {
    let masked = mask_api_key("sk-1234567890abcdefghijklmnop");
    assert!(masked.starts_with("sk-1"));
    assert!(masked.ends_with("mnop"));
    assert!(masked.contains("..."));
    assert!(!masked.contains("567890abcdefghijkl"));
}

#[test]
fn test_short_key_masking() {
    let masked = mask_api_key("short");
    assert_eq!(masked, "****");
}

#[test]
fn test_sanitize_api_error() {
    // Similarly, check verify.
    let sanitized = sanitize_api_error("Invalid API key: sk-1234567890");
    assert!(!sanitized.contains("sk-"));
    assert!(sanitized.contains("authentication"));

    let sanitized = sanitize_api_error("Rate limit exceeded: 100 requests per minute");
    assert!(!sanitized.contains("100"));
    assert!(sanitized.contains("rate limit"));

    let sanitized = sanitize_api_error("Model not found");
    assert_eq!(sanitized, "Model not found");
}

#[test]
fn test_config_debug_masks_key() {
    let config = OpenAiConfig::new("sk-1234567890abcdefghijklmnop");
    let debug_str = format!("{:?}", config);

    assert!(!debug_str.contains("1234567890abcdefghijkl"));
    assert!(debug_str.contains("sk-1...mnop"));
}
