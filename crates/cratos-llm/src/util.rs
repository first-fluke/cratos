//! Common utilities for LLM providers
//!
//! This module contains shared helper functions used across multiple LLM providers
//! to avoid code duplication (DRY principle).

/// Minimum key length to display partial key
const MIN_KEY_LENGTH_FOR_PARTIAL_DISPLAY: usize = 8;

/// Number of characters to show at start/end of masked key
const KEY_MASK_VISIBLE_CHARS: usize = 4;

/// Sensitive patterns to filter from error messages
const SENSITIVE_PATTERNS: &[&str] = &[
    "api_key",
    "api-key",
    "apikey",
    "authorization",
    "bearer",
    "token",
    "secret",
    "password",
    "credential",
];

/// Mask API key for safe display in logs
///
/// Shows first 4 and last 4 characters for keys longer than 8 characters,
/// otherwise shows "****" to prevent exposure of short keys.
///
/// # Examples
/// ```
/// use cratos_llm::util::mask_api_key;
/// assert_eq!(mask_api_key("sk-1234567890abcdef"), "sk-1...cdef");
/// assert_eq!(mask_api_key("short"), "****");
/// ```
#[must_use]
pub fn mask_api_key(key: &str) -> String {
    if key.len() <= MIN_KEY_LENGTH_FOR_PARTIAL_DISPLAY {
        return "****".to_string();
    }
    format!(
        "{}...{}",
        &key[..KEY_MASK_VISIBLE_CHARS],
        &key[key.len() - KEY_MASK_VISIBLE_CHARS..]
    )
}

/// Sanitize error message for user display
///
/// Removes sensitive information from error messages before showing them to users.
/// If the error contains sensitive patterns, returns a generic error message.
///
/// # Examples
/// ```
/// use cratos_llm::util::sanitize_error_for_user;
/// // Error containing "api_key" is sanitized
/// assert_eq!(
///     sanitize_error_for_user("Invalid api_key provided"),
///     "An API error occurred. Please try again."
/// );
/// // Safe error is returned as-is
/// assert_eq!(
///     sanitize_error_for_user("Connection timeout"),
///     "Connection timeout"
/// );
/// ```
#[must_use]
pub fn sanitize_error_for_user(error: &str) -> String {
    let lower = error.to_lowercase();

    // Check for sensitive patterns
    for pattern in SENSITIVE_PATTERNS {
        if lower.contains(pattern) {
            return "An API error occurred. Please try again.".to_string();
        }
    }

    error.to_string()
}

/// Validate API key is not empty and has minimum length
///
/// Returns an error message if validation fails, None if valid.
#[must_use]
pub fn validate_api_key(key: &str, provider_name: &str) -> Option<String> {
    if key.is_empty() {
        return Some(format!("{} API key is required", provider_name));
    }
    if key.len() < MIN_KEY_LENGTH_FOR_PARTIAL_DISPLAY {
        return Some(format!(
            "{} API key appears to be invalid (too short)",
            provider_name
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_api_key_long() {
        let key = "sk-1234567890abcdefghij";
        let masked = mask_api_key(key);
        assert_eq!(masked, "sk-1...ghij");
        assert!(!masked.contains("567890"));
    }

    #[test]
    fn test_mask_api_key_short() {
        assert_eq!(mask_api_key("short"), "****");
        assert_eq!(mask_api_key("12345678"), "****");
    }

    #[test]
    fn test_mask_api_key_empty() {
        assert_eq!(mask_api_key(""), "****");
    }

    #[test]
    fn test_sanitize_error_with_api_key() {
        let error = "Invalid api_key provided";
        let sanitized = sanitize_error_for_user(error);
        assert_eq!(sanitized, "An API error occurred. Please try again.");
    }

    #[test]
    fn test_sanitize_error_with_bearer() {
        let error = "Bearer token expired";
        let sanitized = sanitize_error_for_user(error);
        assert_eq!(sanitized, "An API error occurred. Please try again.");
    }

    #[test]
    fn test_sanitize_error_safe() {
        let error = "Connection timeout after 30s";
        let sanitized = sanitize_error_for_user(error);
        assert_eq!(sanitized, error);
    }

    #[test]
    fn test_validate_api_key() {
        assert!(validate_api_key("", "Test").is_some());
        assert!(validate_api_key("short", "Test").is_some());
        assert!(validate_api_key("valid-api-key-12345", "Test").is_none());
    }
}
