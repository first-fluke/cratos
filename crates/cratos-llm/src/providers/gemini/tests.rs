//! Tests for Gemini provider

use super::config::{downgrade_model, GeminiAuth, GeminiConfig, DEFAULT_BASE_URL, DEFAULT_MODEL};
use super::convert::convert_tools;
use super::schema::strip_unsupported_schema_fields;
use super::security::sanitize_api_error;
use crate::cli_auth::AuthSource;
use crate::router::{Message, ToolDefinition};
use crate::util::mask_api_key;
use std::time::Duration;

#[test]
fn test_config_builder() {
    let config = GeminiConfig::new("test-key")
        .with_model("gemini-1.5-pro")
        .with_max_tokens(4096)
        .with_timeout(Duration::from_secs(30));

    match &config.auth {
        GeminiAuth::ApiKey(key) => assert_eq!(key, "test-key"),
        _ => panic!("Expected ApiKey auth"),
    }
    assert_eq!(config.auth_source, AuthSource::ApiKey);
    assert_eq!(config.default_model, "gemini-1.5-pro");
    assert_eq!(config.default_max_tokens, 4096);
    assert_eq!(config.timeout, Duration::from_secs(30));
}

#[test]
fn test_available_models() {
    use super::config::MODELS;
    assert!(MODELS.contains(&"gemini-3-flash-preview"));
    assert!(MODELS.contains(&"gemini-2.5-flash"));
}

#[test]
fn test_message_conversion() {
    use super::convert::convert_messages;

    let messages = vec![
        Message::system("You are helpful"),
        Message::user("Hello"),
        Message::assistant("Hi there!"),
    ];

    let (system, converted) = convert_messages(&messages);

    assert!(system.is_some());
    assert_eq!(converted.len(), 2);
    assert_eq!(converted[0].role, Some("user".to_string()));
    assert_eq!(converted[1].role, Some("model".to_string()));
}

// Security tests

#[test]
fn test_api_key_masking() {
    let masked = mask_api_key("AIza1234567890abcdefghij");
    assert!(masked.starts_with("AIza"));
    assert!(masked.contains("..."));
    assert!(!masked.contains("1234567890"));
}

#[test]
fn test_sanitize_api_error() {
    let sanitized = sanitize_api_error("Permission denied: invalid API key");
    assert!(!sanitized.contains("invalid"));
    assert!(sanitized.contains("authentication"));

    let sanitized = sanitize_api_error("RESOURCE_EXHAUSTED: quota exceeded");
    assert!(sanitized.contains("rate limit"));
}

#[test]
fn test_config_debug_masks_key() {
    let config = GeminiConfig::new("AIza1234567890abcdefghij");
    let debug_str = format!("{:?}", config);

    assert!(!debug_str.contains("1234567890"));
    assert!(debug_str.contains("AIza...ghij"));
}

#[test]
fn test_config_debug_masks_oauth_token() {
    let config = GeminiConfig {
        auth: GeminiAuth::OAuth("ya29.long-oauth-token-1234567890".to_string()),
        auth_source: AuthSource::GeminiCli,
        base_url: DEFAULT_BASE_URL.to_string(),
        default_model: DEFAULT_MODEL.to_string(),
        default_max_tokens: 8192,
        timeout: Duration::from_secs(60),
        project_id: None,
    };
    let debug_str = format!("{:?}", config);

    assert!(!debug_str.contains("long-oauth-token"));
    assert!(debug_str.contains("OAuth(ya29...7890)"));
}

#[test]
fn test_downgrade_chain() {
    assert_eq!(
        downgrade_model("gemini-3-pro-preview"),
        Some("gemini-3-flash-preview")
    );
    // gemini-3-flash-preview must NOT downgrade to non-thinking model
    // (would cause thought_signature mismatch in conversation history)
    assert_eq!(downgrade_model("gemini-3-flash-preview"), None);
    assert_eq!(downgrade_model("gemini-2.5-pro"), Some("gemini-2.5-flash"));
    assert_eq!(
        downgrade_model("gemini-2.5-flash"),
        Some("gemini-2.5-flash-lite")
    );
    assert_eq!(downgrade_model("gemini-2.5-flash-lite"), None);
}

#[test]
fn test_strip_unsupported_schema_fields() {
    let mut schema = serde_json::json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "File path",
                "default": "/tmp"
            },
            "options": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "recursive": {
                        "type": "boolean",
                        "default": false
                    }
                }
            }
        },
        "required": ["path"],
        "additionalProperties": false
    });

    strip_unsupported_schema_fields(&mut schema);

    let obj = schema.as_object().unwrap();
    // Top-level additionalProperties removed
    assert!(!obj.contains_key("additionalProperties"));
    // Supported fields preserved
    assert!(obj.contains_key("type"));
    assert!(obj.contains_key("properties"));
    assert!(obj.contains_key("required"));

    let path_prop = &schema["properties"]["path"];
    assert_eq!(path_prop.get("type").unwrap(), "string");
    assert_eq!(path_prop.get("description").unwrap(), "File path");
    assert!(path_prop.get("default").is_none());

    let options_prop = &schema["properties"]["options"];
    assert!(options_prop.get("additionalProperties").is_none());

    let recursive_prop = &schema["properties"]["options"]["properties"]["recursive"];
    assert_eq!(recursive_prop.get("type").unwrap(), "boolean");
    assert!(recursive_prop.get("default").is_none());
}

#[test]
fn test_convert_tools_strips_unsupported_fields() {
    let tools = vec![ToolDefinition {
        name: "test_tool".to_string(),
        description: "A test tool".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "default": 10
                }
            }
        }),
    }];

    let gemini_tools = convert_tools(&tools);
    let params = &gemini_tools[0].function_declarations[0].parameters;
    // default should be stripped
    assert!(params["properties"]["count"].get("default").is_none());
    // type should remain
    assert_eq!(params["properties"]["count"]["type"], "integer");
}
