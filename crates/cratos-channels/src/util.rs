//! Common utilities for channel adapters
//!
//! This module contains shared helper functions used across multiple channel adapters
//! to avoid code duplication (DRY principle).

// ============================================================================
// Logging and Security Constants
// ============================================================================

/// Maximum length of text to log (to prevent sensitive data exposure)
pub const MAX_LOG_TEXT_LENGTH: usize = 50;

/// Maximum length of error message to show to users (longer = likely internal)
pub const MAX_SAFE_ERROR_LENGTH: usize = 100;

// ============================================================================
// Platform Message Length Limits
// ============================================================================

/// Discord message character limit
pub const DISCORD_MESSAGE_LIMIT: usize = 2000;

/// WhatsApp message character limit (recommended split size)
pub const WHATSAPP_MESSAGE_LIMIT: usize = 4096;

/// Patterns that indicate potentially sensitive content
pub const SENSITIVE_PATTERNS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "api-key",
    "bearer",
    "authorization",
    "credential",
    "private",
    "ssh",
    "-----begin",
];

/// Mask potentially sensitive text for logging
///
/// Checks for sensitive patterns and truncates long messages
/// to prevent accidental exposure of sensitive data in logs.
///
/// # Examples
/// ```
/// use cratos_channels::util::mask_for_logging;
///
/// // Sensitive content is redacted
/// assert!(mask_for_logging("my password is secret123").contains("REDACTED"));
///
/// // Normal short messages pass through
/// assert_eq!(mask_for_logging("Hello"), "Hello");
/// ```
#[must_use]
pub fn mask_for_logging(text: &str) -> String {
    let lower = text.to_lowercase();

    // Check for sensitive patterns
    for pattern in SENSITIVE_PATTERNS {
        if lower.contains(pattern) {
            return "[REDACTED - potentially sensitive content]".to_string();
        }
    }

    // Truncate long messages
    if text.len() > MAX_LOG_TEXT_LENGTH {
        format!("{}...[truncated]", &text[..MAX_LOG_TEXT_LENGTH])
    } else {
        text.to_string()
    }
}

/// Sanitize error messages to avoid exposing internal details
///
/// Removes or masks potentially sensitive information from error messages
/// before showing them to users via messaging channels.
///
/// # Examples
/// ```
/// use cratos_channels::util::sanitize_error_for_user;
///
/// // Auth errors are sanitized
/// let sanitized = sanitize_error_for_user("Invalid token: abc123");
/// assert!(!sanitized.contains("abc123"));
///
/// // Simple safe errors pass through
/// assert_eq!(sanitize_error_for_user("File not found"), "File not found");
/// ```
#[must_use]
pub fn sanitize_error_for_user(error: &str) -> String {
    let lower = error.to_lowercase();

    // Hide authentication-related errors
    if lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
    {
        return "An authentication error occurred. Please check your configuration.".to_string();
    }

    // Hide network errors
    if lower.contains("connection") || lower.contains("timeout") || lower.contains("network") {
        return "A network error occurred. Please try again later.".to_string();
    }

    // Hide database errors
    if lower.contains("database") || lower.contains("sql") || lower.contains("query") {
        return "A database error occurred. Please try again later.".to_string();
    }

    // Hide internal errors (paths, stack traces)
    if error.len() > MAX_SAFE_ERROR_LENGTH || error.contains('/') || error.contains("at ") {
        return "An internal error occurred. Please try again.".to_string();
    }

    // Short, non-sensitive errors can be shown
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_for_logging_sensitive() {
        assert!(mask_for_logging("my password is secret123").contains("REDACTED"));
        assert!(mask_for_logging("API_KEY=sk-1234567890").contains("REDACTED"));
        assert!(mask_for_logging("Bearer eyJhbGciOiJ").contains("REDACTED"));
        assert!(mask_for_logging("-----BEGIN RSA PRIVATE KEY-----").contains("REDACTED"));
    }

    #[test]
    fn test_mask_for_logging_truncate() {
        let long_msg = "a".repeat(100);
        let masked = mask_for_logging(&long_msg);
        assert!(masked.contains("truncated"));
        assert!(masked.len() < long_msg.len());
    }

    #[test]
    fn test_mask_for_logging_pass_through() {
        assert_eq!(mask_for_logging("Hello, world!"), "Hello, world!");
        assert_eq!(mask_for_logging("요약해줘"), "요약해줘");
    }

    #[test]
    fn test_sanitize_error_auth() {
        let sanitized = sanitize_error_for_user("Invalid token: abc123");
        assert!(!sanitized.contains("abc123"));
        assert!(sanitized.contains("authentication"));
    }

    #[test]
    fn test_sanitize_error_database() {
        let sanitized = sanitize_error_for_user("SQL error: SELECT * FROM users");
        assert!(!sanitized.contains("SELECT"));
        assert!(sanitized.contains("database"));
    }

    #[test]
    fn test_sanitize_error_internal() {
        let sanitized =
            sanitize_error_for_user("Error at /home/user/.config/app/config.json line 42");
        assert!(!sanitized.contains("/home"));
        assert!(sanitized.to_lowercase().contains("internal"));
    }

    #[test]
    fn test_sanitize_error_pass_through() {
        let simple = sanitize_error_for_user("File not found");
        assert_eq!(simple, "File not found");
    }
}
