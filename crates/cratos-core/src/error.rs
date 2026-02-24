//! Error types for cratos-core
//!
//! This module provides error types and user-friendly error formatting.

use thiserror::Error;

/// Chronicles/History error
#[derive(Debug, Error)]
pub enum ChronicleError {
    /// Database error
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Serialization/deserialization error
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Chronicle record not found
    #[error("chronicle not found: {0}")]
    NotFound(String),

    /// Invalid data format
    #[error("invalid data: {0}")]
    InvalidData(String),

    /// File I/O error
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Scheduler storage error
#[derive(Debug, Error)]
pub enum SchedulerStoreError {
    /// Database error
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Serialization/deserialization error
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Scheduled task not found
    #[error("task not found: {0}")]
    TaskNotFound(uuid::Uuid),

    /// Trigger expression parsing error
    #[error("trigger parse error: {0}")]
    TriggerParse(String),

    /// Database transaction error
    #[error("transaction error: {0}")]
    Transaction(String),

    /// Invalid configuration
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Task execution error
    #[error("execution error: {0}")]
    Execution(String),
}

/// Session/Memory storage error
#[derive(Debug, Error)]
pub enum MemoryStoreError {
    /// SQLite database operation failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// JSON serialization/deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Requested memory record not found.
    #[error("memory record not found: {0}")]
    NotFound(String),

    /// In-memory cache operation failed.
    #[error("cache error: {0}")]
    Cache(String),

    /// Mutex lock was poisoned by a panicking thread.
    #[error("lock poisoned")]
    Poisoned,

    /// Unrecoverable internal error.
    #[error("internal error: {0}")]
    Internal(String),

    /// File system I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

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
    Memory(#[from] MemoryStoreError),

    /// Chronicles/History error
    #[error("chronicle error: {0}")]
    Chronicle(#[from] ChronicleError),

    /// Scheduler error
    #[error("scheduler error: {0}")]
    Scheduler(#[from] SchedulerStoreError),

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

    /// Resource not found
    #[error("not found: {0}")]
    NotFound(String),

    /// Unauthorized access
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Invalid state for the requested operation
    #[error("invalid state: {0}")]
    InvalidState(String),

    /// Internal error (Redis, serialization, etc.)
    #[error("internal error: {0}")]
    Internal(String),

    /// Execuction was aborted by steering
    #[error("execution aborted: {0}")]
    Aborted(String),

    /// Detailed Steering Error
    #[error("steering error: {0}")]
    Steering(#[from] crate::steering::SteerError),
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
                format!("⚙️ Configuration error in \"{}\": {}", field, message)
            }
            Error::Planning(msg) => {
                format!("📋 Planning failed: {}", msg)
            }
            Error::Execution(msg) => {
                format!("⚡ Execution failed: {}", msg)
            }
            Error::Memory(e) => {
                format!("🧠 Memory error: {}", e)
            }
            Error::Chronicle(e) => {
                format!("📜 Chronicle error: {}", e)
            }
            Error::Scheduler(e) => {
                format!("📅 Scheduler error: {}", e)
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
            Error::NotFound(msg) => {
                format!("🔍 Not found: {}", msg)
            }
            Error::Unauthorized(msg) => {
                format!("🔒 Unauthorized: {}", msg)
            }
            Error::InvalidState(msg) => {
                format!("⚠️ Invalid state: {}", msg)
            }
            Error::Internal(msg) => {
                format!("❌ Internal error: {}", msg)
            }
            Error::Aborted(msg) => {
                format!("🛑 Execution aborted: {}", msg)
            }
            Error::Steering(e) => {
                format!("🛑 Steering error: {}", e)
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
                "💡 Check the \"{}\" setting in config/default.toml or .env file.",
                field
            )),
            Error::Planning(_) => {
                Some("💡 Try breaking down your request into smaller steps.".to_string())
            }
            Error::Execution(_) => Some("💡 Check the tool parameters and try again.".to_string()),
            Error::Memory(MemoryStoreError::Database(_)) | 
            Error::Chronicle(ChronicleError::Database(_)) | 
            Error::Scheduler(SchedulerStoreError::Database(_)) => {
                Some("💡 Database connection issue. Check if SQLite/Postgres is running.".to_string())
            }
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
            Error::Memory(_) => Some("https://docs.cratos.dev/architecture/memory"),
            Error::Chronicle(_) => Some("https://docs.cratos.dev/architecture/chronicles"),
            Error::Scheduler(_) => Some("https://docs.cratos.dev/usage/scheduler"),
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
mod tests;

