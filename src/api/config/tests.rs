use super::*;

#[test]
fn test_valid_providers() {
    assert!(is_valid_provider("openai"));
    assert!(is_valid_provider("anthropic"));
    assert!(is_valid_provider("google_pro"));
    assert!(is_valid_provider("groq"));
    assert!(!is_valid_provider("invalid"));
}

#[test]
fn test_valid_languages() {
    assert!(is_valid_language("en"));
    assert!(is_valid_language("ko"));
    assert!(!is_valid_language("xx"));
}

#[test]
fn test_valid_personas() {
    assert!(is_valid_persona("cratos"));
    assert!(is_valid_persona("sindri"));
    assert!(!is_valid_persona("zeus"));
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
