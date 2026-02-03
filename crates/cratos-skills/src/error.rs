//! Error types for cratos-skills.
//!
//! This module provides error types for all skill system operations.
//!
//! # Error Categories
//!
//! | Error | Description |
//! |-------|-------------|
//! | [`Error::SkillNotFound`] | Skill ID or name not found in store |
//! | [`Error::PatternNotFound`] | Pattern ID not found in store |
//! | [`Error::Database`] | SQLite operation failed |
//! | [`Error::Serialization`] | JSON serialization/deserialization failed |
//! | [`Error::Validation`] | Input validation failed (e.g., too many steps) |
//! | [`Error::Execution`] | Skill execution failed |
//! | [`Error::Configuration`] | Invalid configuration |
//! | [`Error::Io`] | File system error |
//! | [`Error::ReplayStore`] | cratos-replay EventStore error |
//! | [`Error::Internal`] | Internal error (semantic search, etc.) |
//!
//! # Example
//!
//! ```ignore
//! use cratos_skills::{Error, Result};
//!
//! fn validate_skill_name(name: &str) -> Result<()> {
//!     if name.is_empty() {
//!         return Err(Error::Validation("skill name cannot be empty".to_string()));
//!     }
//!     if name.len() > 100 {
//!         return Err(Error::Validation("skill name too long (max 100 chars)".to_string()));
//!     }
//!     Ok(())
//! }
//! ```

use thiserror::Error;

/// Skill system error type.
///
/// All skill system operations return [`Result<T>`](type.Result.html) which uses this error type.
#[derive(Debug, Error)]
pub enum Error {
    /// Skill not found in store.
    ///
    /// The string contains the skill ID or name that was not found.
    #[error("skill not found: {0}")]
    SkillNotFound(String),

    /// Pattern not found in store.
    ///
    /// The string contains the pattern ID that was not found.
    #[error("pattern not found: {0}")]
    PatternNotFound(String),

    /// Database operation failed.
    ///
    /// Includes SQLite connection errors, query errors, and constraint violations.
    #[error("database error: {0}")]
    Database(String),

    /// JSON serialization or deserialization failed.
    ///
    /// Occurs when skill data cannot be converted to/from JSON format.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Input validation failed.
    ///
    /// Includes security checks like step count limits and variable size limits.
    #[error("validation error: {0}")]
    Validation(String),

    /// Skill execution failed.
    ///
    /// Includes tool execution errors and variable interpolation failures.
    #[error("execution error: {0}")]
    Execution(String),

    /// Invalid configuration provided.
    #[error("configuration error: {0}")]
    Configuration(String),

    /// File system I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// cratos-replay EventStore error.
    ///
    /// Occurs during pattern analysis when reading execution history.
    #[error("replay store error: {0}")]
    ReplayStore(#[from] cratos_replay::Error),

    /// Internal error.
    ///
    /// Includes semantic search errors and other internal failures.
    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Returns `true` if this is a not-found error.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::SkillNotFound(_) | Error::PatternNotFound(_))
    }

    /// Returns `true` if this is a validation error.
    #[must_use]
    pub fn is_validation(&self) -> bool {
        matches!(self, Error::Validation(_))
    }

    /// Returns `true` if this is a database error.
    #[must_use]
    pub fn is_database(&self) -> bool {
        matches!(self, Error::Database(_))
    }
}

/// Result type alias for skill system operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_is_not_found() {
        assert!(Error::SkillNotFound("test".to_string()).is_not_found());
        assert!(Error::PatternNotFound("test".to_string()).is_not_found());
        assert!(!Error::Validation("test".to_string()).is_not_found());
    }

    #[test]
    fn test_error_is_validation() {
        assert!(Error::Validation("test".to_string()).is_validation());
        assert!(!Error::Database("test".to_string()).is_validation());
    }

    #[test]
    fn test_error_display() {
        let err = Error::SkillNotFound("my_skill".to_string());
        assert_eq!(err.to_string(), "skill not found: my_skill");
    }
}
