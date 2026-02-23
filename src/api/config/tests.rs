use super::*;
use crate::server::config::ConfigValidator;

#[test]
fn test_valid_providers() {
    assert!(ConfigValidator::validate_provider("openai").is_ok());
    assert!(ConfigValidator::validate_provider("anthropic").is_ok());
    assert!(ConfigValidator::validate_provider("google_pro").is_ok());
    assert!(ConfigValidator::validate_provider("groq").is_ok());
    assert!(ConfigValidator::validate_provider("glm").is_ok());
    assert!(ConfigValidator::validate_provider("moonshot").is_ok());
    assert!(ConfigValidator::validate_provider("qwen").is_ok());
    assert!(ConfigValidator::validate_provider("invalid").is_err());
}

#[test]
fn test_valid_languages() {
    assert!(ConfigValidator::validate_language("en").is_ok());
    assert!(ConfigValidator::validate_language("ko").is_ok());
    assert!(ConfigValidator::validate_language("xx").is_err());
}

#[test]
fn test_valid_personas() {
    assert!(ConfigValidator::validate_persona("cratos").is_ok());
    assert!(ConfigValidator::validate_persona("sindri").is_ok());
    assert!(ConfigValidator::validate_persona("thor").is_ok());
    assert!(ConfigValidator::validate_persona("zeus").is_err());
}

#[test]
fn test_valid_approval_modes() {
    assert!(ConfigValidator::validate_approval_mode("always").is_ok());
    assert!(ConfigValidator::validate_approval_mode("risky_only").is_ok());
    assert!(ConfigValidator::validate_approval_mode("never").is_ok());
    assert!(ConfigValidator::validate_approval_mode("invalid").is_err());
}

#[test]
fn test_valid_exec_modes() {
    assert!(ConfigValidator::validate_exec_mode("permissive").is_ok());
    assert!(ConfigValidator::validate_exec_mode("strict").is_ok());
    assert!(ConfigValidator::validate_exec_mode("invalid").is_err());
}

#[test]
fn test_valid_port() {
    assert!(ConfigValidator::validate_port(19527).is_ok());
    assert!(ConfigValidator::validate_port(0).is_err());
}

#[test]
fn test_api_response_success() {
    let response: ApiResponse<String> = ApiResponse::success("test".to_string());
    assert!(response.success);
    assert_eq!(response.data, Some("test".to_string()));
    assert!(response.error.is_none());
}

#[test]
fn test_api_response_error() {
    let response: ApiResponse<String> = ApiResponse::error("error message");
    assert!(!response.success);
    assert!(response.data.is_none());
    assert_eq!(response.error, Some("error message".to_string()));
}

#[test]
fn test_app_config_view_from_default() {
    let config = AppConfig::default();
    let view = AppConfigView::from(config);
    assert_eq!(view.general.language, "auto");
    assert_eq!(view.general.persona, "cratos");
    assert_eq!(view.llm.default_provider, "auto");
    assert_eq!(view.security.approval_mode, "never");
}
