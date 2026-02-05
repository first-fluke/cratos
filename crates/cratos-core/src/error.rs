//! Error types for cratos-core
//!
//! This module provides error types and user-friendly error formatting.

use thiserror::Error;

/// Core error type
#[derive(Debug, Error)]
pub enum Error {
    /// API key missing or invalid
    #[error("API key error: {provider}")]
    ApiKeyMissing {
        /// Provider name
        provider: String,
    },

    /// Network/connection error
    #[error("network error: {0}")]
    NetworkError(String),

    /// Rate limit exceeded
    #[error("rate limit exceeded")]
    RateLimited {
        /// Seconds until retry is allowed
        retry_after: Option<u64>,
    },

    /// Invalid configuration
    #[error("invalid configuration: {field}")]
    InvalidConfig {
        /// Config field name
        field: String,
        /// Detailed message
        message: String,
    },

    /// Planning failed
    #[error("planning error: {0}")]
    Planning(String),

    /// Execution failed
    #[error("execution error: {0}")]
    Execution(String),

    /// Memory error
    #[error("memory error: {0}")]
    Memory(String),

    /// Approval timeout or rejection
    #[error("approval error: {0}")]
    Approval(String),

    /// Configuration error (invalid settings, production safety violations)
    #[error("configuration error: {0}")]
    Configuration(String),

    /// LLM provider error
    #[error("llm error: {0}")]
    Llm(#[from] cratos_llm::Error),

    /// Tool execution error
    #[error("tool error: {0}")]
    Tool(#[from] cratos_tools::Error),

    /// Replay/logging error
    #[error("replay error: {0}")]
    Replay(#[from] cratos_replay::Error),

    /// Internal error (Redis, serialization, etc.)
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Trait for user-friendly error messages
///
/// Provides human-readable error messages, suggestions for fixing,
/// and links to documentation.
pub trait UserFriendlyError {
    /// Get a user-friendly error message
    fn user_message(&self) -> String;

    /// Get a suggestion for how to fix the error
    fn suggestion(&self) -> Option<String>;

    /// Get a documentation URL for more information
    fn docs_url(&self) -> Option<&str>;
}

impl UserFriendlyError for Error {
    fn user_message(&self) -> String {
        match self {
            Error::ApiKeyMissing { provider } => {
                format!("🔑 {} API key is not configured.", provider)
            }
            Error::NetworkError(_) => "🌐 Network connection problem.".to_string(),
            Error::RateLimited { retry_after } => {
                if let Some(secs) = retry_after {
                    format!("⏳ Rate limit exceeded. Please wait {} seconds.", secs)
                } else {
                    "⏳ Rate limit exceeded. Please try again later.".to_string()
                }
            }
            Error::InvalidConfig { field, message } => {
                format!("⚙️ Configuration error in '{}': {}", field, message)
            }
            Error::Planning(msg) => {
                format!("📋 Planning failed: {}", msg)
            }
            Error::Execution(msg) => {
                format!("⚡ Execution failed: {}", msg)
            }
            Error::Memory(msg) => {
                format!("🧠 Memory error: {}", msg)
            }
            Error::Approval(msg) => {
                format!("✋ Approval required: {}", msg)
            }
            Error::Configuration(msg) => {
                format!("⚙️ Configuration error: {}", msg)
            }
            Error::Llm(e) => {
                format!("🤖 LLM error: {}", e)
            }
            Error::Tool(e) => {
                format!("🔧 Tool error: {}", e)
            }
            Error::Replay(e) => {
                format!("📼 Replay error: {}", e)
            }
            Error::Internal(msg) => {
                format!("❌ Internal error: {}", msg)
            }
        }
    }

    fn suggestion(&self) -> Option<String> {
        match self {
            Error::ApiKeyMissing { provider } => Some(format!(
                "💡 Run `cratos init` or set the {}_API_KEY environment variable.",
                provider.to_uppercase().replace(' ', "_")
            )),
            Error::NetworkError(_) => {
                Some("💡 Check your internet connection and firewall settings.".to_string())
            }
            Error::RateLimited { .. } => {
                Some("💡 Try using a different model or wait before retrying.".to_string())
            }
            Error::InvalidConfig { field, .. } => Some(format!(
                "💡 Check the '{}' setting in config/default.toml or .env file.",
                field
            )),
            Error::Planning(_) => {
                Some("💡 Try breaking down your request into smaller steps.".to_string())
            }
            Error::Execution(_) => Some("💡 Check the tool parameters and try again.".to_string()),
            Error::Approval(_) => {
                Some("💡 Review the pending approval and respond with 'yes' or 'no'.".to_string())
            }
            Error::Configuration(_) => {
                Some("💡 Run `cratos init` to reconfigure settings.".to_string())
            }
            _ => None,
        }
    }

    fn docs_url(&self) -> Option<&str> {
        match self {
            Error::ApiKeyMissing { .. } => Some("https://docs.cratos.dev/setup/api-keys"),
            Error::NetworkError(_) => Some("https://docs.cratos.dev/troubleshooting/network"),
            Error::RateLimited { .. } => Some("https://docs.cratos.dev/usage/rate-limits"),
            Error::InvalidConfig { .. } | Error::Configuration(_) => {
                Some("https://docs.cratos.dev/configuration")
            }
            Error::Approval(_) => Some("https://docs.cratos.dev/security/approvals"),
            _ => None,
        }
    }
}

/// Format an error for display in the CLI
pub fn format_error_for_cli(error: &Error) -> String {
    let mut output = String::new();

    // User-friendly message
    output.push_str(&error.user_message());
    output.push_str("\n\n");

    // Suggestion
    if let Some(suggestion) = error.suggestion() {
        output.push_str(&suggestion);
        output.push_str("\n\n");
    }

    // Documentation link
    if let Some(url) = error.docs_url() {
        output.push_str(&format!("📚 More info: {}", url));
        output.push('\n');
    }

    output
}

/// Format an error for display in a chat message
pub fn format_error_for_chat(error: &Error) -> String {
    let mut output = error.user_message();

    if let Some(suggestion) = error.suggestion() {
        output.push_str("\n\n");
        output.push_str(&suggestion);
    }

    output
}

#[cfg(test)]
mod tests {
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
}
