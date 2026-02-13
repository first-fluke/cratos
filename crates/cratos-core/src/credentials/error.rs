//! Credential error types

use thiserror::Error;

/// Credential store errors
#[derive(Debug, Error)]
pub enum CredentialError {
    /// Credential not found
    #[error("Credential not found: {0}")]
    NotFound(String),

    /// Backend error
    #[error("Backend error: {0}")]
    Backend(String),

    /// Encryption error
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Access denied
    #[error("Access denied: {0}")]
    AccessDenied(String),
}

/// Result type for credential operations
pub type Result<T> = std::result::Result<T, CredentialError>;

/// Handle RwLock poison errors consistently
pub fn handle_lock_poison<T>(e: std::sync::PoisonError<T>) -> CredentialError {
    CredentialError::Backend(format!("Lock poisoned: {}", e))
}

/// Generate credential key from service and account
pub fn credential_key(service: &str, account: &str) -> String {
    format!("{}:{}", service, account)
}
