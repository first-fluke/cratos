use super::{SessionContext, SessionStore};
use crate::error::MemoryStoreError;
pub type Result<T> = std::result::Result<T, MemoryStoreError>;
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
/// Check if running in production environment
fn is_production() -> bool {
    std::env::var("CRATOS_ENV")
        .map(|v| v.to_lowercase() == "production")
        .unwrap_or(false)
}

/// Check if production safety bypass is enabled
fn is_production_bypass_enabled() -> bool {
    std::env::var("CRATOS_ALLOW_MEMORY_STORE_IN_PRODUCTION")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}


/// In-memory session store (for development/testing)
///
/// # Security Warning
///
/// This store is NOT suitable for production use:
/// - Data is lost on application restart
/// - No persistence or replication
/// - No encryption at rest
/// - Memory can grow unbounded without proper cleanup
///
/// Use `RedisStore` for production deployments.
///
/// # Production Safety
///
/// In production (`CRATOS_ENV=production`), `new()` returns an error.
/// Use `new_unsafe()` or set `CRATOS_ALLOW_MEMORY_STORE_IN_PRODUCTION=1` to bypass.
pub struct MemoryStore {
    sessions: Arc<RwLock<HashMap<String, SessionContext>>>,
    /// Session TTL in hours
    ttl_hours: u64,
    /// Whether production safety was bypassed
    production_bypass: bool,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::try_new().expect("MemoryStore not allowed in production. Use RedisStore or set CRATOS_ALLOW_MEMORY_STORE_IN_PRODUCTION=1")
    }
}

impl MemoryStore {
    /// Create a new memory store
    ///
    /// # Errors
    ///
    /// Returns error if `CRATOS_ENV=production` unless bypass is enabled.
    ///
    /// # Production Safety
    ///
    /// - In production: Returns `Err(Error::Configuration(...))`
    /// - To bypass: Set `CRATOS_ALLOW_MEMORY_STORE_IN_PRODUCTION=1`
    /// - Or use `new_unsafe()` (not recommended)
    pub fn try_new() -> Result<Self> {
        if is_production() && !is_production_bypass_enabled() {
            error!(
                "SECURITY BLOCK: MemoryStore is not allowed in production. \
                 Use RedisStore for production deployments. \
                 To bypass (NOT RECOMMENDED), set CRATOS_ALLOW_MEMORY_STORE_IN_PRODUCTION=1"
            );
            return Err(MemoryStoreError::Internal(
                "MemoryStore is not allowed in production. Use RedisStore instead.".to_string(),
            ));
        }

        if is_production() {
            warn!(
                "SECURITY WARNING: MemoryStore is being used in production with safety bypass. \
                 This is not recommended - use RedisStore instead for data persistence and security."
            );
        }

        info!("Initializing MemoryStore for session storage");

        Ok(Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            ttl_hours: 24,
            production_bypass: is_production(),
        })
    }

    /// Create a new memory store (legacy API, panics in production)
    ///
    /// # Panics
    ///
    /// Panics if `CRATOS_ENV=production` unless bypass is enabled.
    ///
    /// # Deprecated
    ///
    /// Use `try_new()` instead for better error handling.
    #[must_use]
    pub fn new() -> Self {
        Self::try_new().expect(
            "MemoryStore not allowed in production. Use RedisStore or set CRATOS_ALLOW_MEMORY_STORE_IN_PRODUCTION=1"
        )
    }

    /// Create a new memory store, bypassing production safety checks
    ///
    /// # Security Warning
    ///
    /// This bypasses production safety checks. Only use when:
    /// - You have a specific, documented reason
    /// - You understand the security implications
    /// - There is no alternative (e.g., Redis is unavailable)
    ///
    /// Production use without persistence means:
    /// - All sessions lost on restart
    /// - No data recovery possible
    /// - Potential data loss for users
    #[must_use]
    pub fn new_unsafe() -> Self {
        if is_production() {
            warn!(
                "SECURITY WARNING: MemoryStore::new_unsafe() called in production. \
                 Session data will not persist across restarts. \
                 This should only be used when Redis is unavailable."
            );
        }

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            ttl_hours: 24,
            production_bypass: true,
        }
    }

    /// Create with custom TTL
    ///
    /// # Errors
    ///
    /// Returns error if `CRATOS_ENV=production` unless bypass is enabled.
    pub fn try_with_ttl_hours(ttl_hours: u64) -> Result<Self> {
        let mut store = Self::try_new()?;
        store.ttl_hours = ttl_hours;
        Ok(store)
    }

    /// Create with custom TTL (legacy API, panics in production)
    ///
    /// # Panics
    ///
    /// Panics if `CRATOS_ENV=production` unless bypass is enabled.
    #[must_use]
    pub fn with_ttl_hours(ttl_hours: u64) -> Self {
        Self::try_with_ttl_hours(ttl_hours).expect(
            "MemoryStore not allowed in production. Use RedisStore or set CRATOS_ALLOW_MEMORY_STORE_IN_PRODUCTION=1"
        )
    }

    /// Check if this store is running with production safety bypass
    #[must_use]
    pub fn is_production_bypass(&self) -> bool {
        self.production_bypass
    }

    /// Check if this store is safe for the current environment
    #[must_use]
    pub fn is_production_safe(&self) -> bool {
        !is_production()
    }

    /// Get or create a session (synchronous convenience method)
    pub async fn get_or_create(&self, session_key: &str) -> SessionContext {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_key) {
            return session.clone();
        }
        drop(sessions);

        let mut sessions = self.sessions.write().await;
        sessions
            .entry(session_key.to_string())
            .or_insert_with(|| SessionContext::new(session_key))
            .clone()
    }
}

#[async_trait]
impl SessionStore for MemoryStore {
    async fn get(&self, session_key: &str) -> Result<Option<SessionContext>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_key).cloned())
    }

    async fn save(&self, session: &SessionContext) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.session_key.clone(), session.clone());
        Ok(())
    }

    async fn delete(&self, session_key: &str) -> Result<bool> {
        let mut sessions = self.sessions.write().await;
        Ok(sessions.remove(session_key).is_some())
    }

    async fn exists(&self, session_key: &str) -> Result<bool> {
        let sessions = self.sessions.read().await;
        Ok(sessions.contains_key(session_key))
    }

    async fn list_keys(&self) -> Result<Vec<String>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.keys().cloned().collect())
    }

    async fn count(&self) -> Result<usize> {
        let sessions = self.sessions.read().await;
        Ok(sessions.len())
    }

    async fn cleanup_expired(&self) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::hours(self.ttl_hours as i64);
        let mut sessions = self.sessions.write().await;
        let initial_count = sessions.len();

        // SECURITY: Collect keys to remove first, then explicitly clear session data
        let expired_keys: Vec<String> = sessions
            .iter()
            .filter(|(_, session)| session.last_activity <= cutoff)
            .map(|(key, _)| key.clone())
            .collect();

        let removed_count = expired_keys.len();

        for key in expired_keys {
            if let Some(mut session) = sessions.remove(&key) {
                // SECURITY: Explicitly clear sensitive data before dropping
                session.messages.clear();
                session.metadata.clear();
                debug!(session_key = %key, "Expired session data cleared and removed");
            }
        }

        if removed_count > 0 {
            debug!(
                removed = removed_count,
                remaining = initial_count - removed_count,
                "Cleaned up expired sessions"
            );
        }

        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests;
