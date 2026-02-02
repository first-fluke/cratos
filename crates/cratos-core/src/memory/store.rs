//! Session storage backends
//!
//! Provides both in-memory and Redis-backed session storage.

use super::SessionContext;
use crate::error::{Error, Result};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

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

/// In-memory session store (for development/testing)
pub struct MemoryStore {
    sessions: Arc<RwLock<HashMap<String, SessionContext>>>,
    /// Session TTL in hours
    ttl_hours: u64,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    /// Create a new memory store
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            ttl_hours: 24,
        }
    }

    /// Create with custom TTL
    #[must_use]
    pub fn with_ttl_hours(ttl_hours: u64) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            ttl_hours,
        }
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
        sessions.retain(|_, session| session.last_activity > cutoff);
        Ok(initial_count - sessions.len())
    }
}

/// Redis-backed session store (for production)
pub struct RedisStore {
    client: redis::Client,
    /// Key prefix for session keys
    prefix: String,
    /// TTL in seconds
    ttl_seconds: u64,
}

impl RedisStore {
    /// Create a new Redis store
    ///
    /// # Errors
    ///
    /// Returns error if Redis URL is invalid
    pub fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url).map_err(|e| Error::Internal(e.to_string()))?;

        Ok(Self {
            client,
            prefix: "cratos:session:".to_string(),
            ttl_seconds: 24 * 3600, // 24 hours
        })
    }

    /// Create with custom prefix and TTL
    ///
    /// # Errors
    ///
    /// Returns error if Redis URL is invalid
    pub fn with_options(redis_url: &str, prefix: &str, ttl_seconds: u64) -> Result<Self> {
        let client = redis::Client::open(redis_url).map_err(|e| Error::Internal(e.to_string()))?;

        Ok(Self {
            client,
            prefix: prefix.to_string(),
            ttl_seconds,
        })
    }

    /// Build the full Redis key
    fn build_key(&self, session_key: &str) -> String {
        format!("{}{}", self.prefix, session_key)
    }

    /// Get an async connection
    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| Error::Internal(format!("Redis connection failed: {}", e)))
    }
}

#[async_trait]
impl SessionStore for RedisStore {
    async fn get(&self, session_key: &str) -> Result<Option<SessionContext>> {
        let mut conn = self.get_connection().await?;
        let key = self.build_key(session_key);

        let data: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis GET failed: {}", e)))?;

        match data {
            Some(json) => {
                let session: SessionContext = serde_json::from_str(&json).map_err(|e| {
                    Error::Internal(format!("Failed to deserialize session: {}", e))
                })?;
                debug!(session_key = %session_key, "Session loaded from Redis");
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    async fn save(&self, session: &SessionContext) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let key = self.build_key(&session.session_key);

        let json = serde_json::to_string(session)
            .map_err(|e| Error::Internal(format!("Failed to serialize session: {}", e)))?;

        redis::cmd("SETEX")
            .arg(&key)
            .arg(self.ttl_seconds)
            .arg(&json)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis SETEX failed: {}", e)))?;

        debug!(session_key = %session.session_key, ttl = %self.ttl_seconds, "Session saved to Redis");
        Ok(())
    }

    async fn delete(&self, session_key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        let key = self.build_key(session_key);

        let deleted: i64 = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis DEL failed: {}", e)))?;

        debug!(session_key = %session_key, deleted = deleted > 0, "Session deleted from Redis");
        Ok(deleted > 0)
    }

    async fn exists(&self, session_key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        let key = self.build_key(session_key);

        let exists: i64 = redis::cmd("EXISTS")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis EXISTS failed: {}", e)))?;

        Ok(exists > 0)
    }

    async fn list_keys(&self) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;
        let pattern = format!("{}*", self.prefix);

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Internal(format!("Redis KEYS failed: {}", e)))?;

        // Strip prefix from keys
        let session_keys: Vec<String> = keys
            .into_iter()
            .filter_map(|k| k.strip_prefix(&self.prefix).map(|s| s.to_string()))
            .collect();

        warn!(
            count = session_keys.len(),
            "Listed all session keys from Redis (use with caution)"
        );
        Ok(session_keys)
    }

    async fn count(&self) -> Result<usize> {
        // Note: This is not efficient for large datasets
        // Consider using SCAN in production
        let keys = self.list_keys().await?;
        Ok(keys.len())
    }

    async fn cleanup_expired(&self) -> Result<usize> {
        // Redis TTL handles expiration automatically
        // This is a no-op but could scan for stale keys if needed
        debug!("Redis TTL handles session expiration automatically");
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_store() {
        let store = MemoryStore::new();

        // Initially empty
        assert_eq!(store.count().await.unwrap(), 0);

        // Create and save session
        let mut session = SessionContext::new("test:key");
        session.add_user_message("Hello");
        store.save(&session).await.unwrap();

        // Verify saved
        assert!(store.exists("test:key").await.unwrap());
        assert_eq!(store.count().await.unwrap(), 1);

        // Retrieve
        let loaded = store.get("test:key").await.unwrap().unwrap();
        assert_eq!(loaded.message_count(), 1);

        // Delete
        assert!(store.delete("test:key").await.unwrap());
        assert!(!store.exists("test:key").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_store_get_or_create() {
        let store = MemoryStore::new();

        // First call creates
        let session1 = store.get_or_create("new:key").await;
        assert_eq!(session1.session_key, "new:key");

        // Second call returns same
        let session2 = store.get_or_create("new:key").await;
        assert_eq!(session1.id, session2.id);
    }

    #[tokio::test]
    async fn test_memory_store_list_keys() {
        let store = MemoryStore::new();

        store.save(&SessionContext::new("key1")).await.unwrap();
        store.save(&SessionContext::new("key2")).await.unwrap();
        store.save(&SessionContext::new("key3")).await.unwrap();

        let keys = store.list_keys().await.unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    // Redis tests require a running Redis instance
    // Run with: cargo test --features redis-tests
    #[cfg(feature = "redis-tests")]
    mod redis_tests {
        use super::*;

        #[tokio::test]
        async fn test_redis_store() {
            let store = RedisStore::new("redis://127.0.0.1:6379").unwrap();

            let mut session = SessionContext::new("test:redis:key");
            session.add_user_message("Hello from Redis");

            store.save(&session).await.unwrap();

            let loaded = store.get("test:redis:key").await.unwrap().unwrap();
            assert_eq!(loaded.message_count(), 1);

            store.delete("test:redis:key").await.unwrap();
        }
    }
}
