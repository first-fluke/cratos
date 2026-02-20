
use async_trait::async_trait;
use crate::error::MemoryStoreError;
use super::SessionContext;
pub type Result<T> = std::result::Result<T, MemoryStoreError>;

/// Session store trait for abstracting storage backends
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Get a session by key
    async fn get(&self, session_key: &str) -> Result<Option<SessionContext>>;

    /// Save a session
    async fn save(&self, session: &SessionContext) -> Result<()>;

    /// Delete a session
    async fn delete(&self, session_key: &str) -> Result<bool>;

    /// Check if a session exists
    async fn exists(&self, session_key: &str) -> Result<bool>;

    /// List all session keys (use with caution in production)
    async fn list_keys(&self) -> Result<Vec<String>>;

    /// Get session count
    async fn count(&self) -> Result<usize>;

    /// Cleanup expired sessions
    async fn cleanup_expired(&self) -> Result<usize>;
}

