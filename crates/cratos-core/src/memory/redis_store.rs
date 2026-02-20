use super::{SessionContext, SessionStore};
use crate::error::MemoryStoreError;
pub type Result<T> = std::result::Result<T, MemoryStoreError>;
use async_trait::async_trait;
use tracing::{debug, error, info, warn};
/// Redis-backed session store (for production)
///
/// # Security Features
///
/// - Automatic TTL-based expiration
/// - Session keys are prefixed to isolate from other Redis data
/// - Consider enabling Redis AUTH and TLS in production
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
        let client = redis::Client::open(redis_url).map_err(|e| MemoryStoreError::Internal(e.to_string()))?;

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
        let client = redis::Client::open(redis_url).map_err(|e| MemoryStoreError::Internal(e.to_string()))?;

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
            .map_err(|e| MemoryStoreError::Internal(format!("Redis connection failed: {}", e)))
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
            .map_err(|e| MemoryStoreError::Internal(format!("Redis GET failed: {}", e)))?;

        match data {
            Some(json) => {
                let session: SessionContext = serde_json::from_str(&json).map_err(|e| {
                    MemoryStoreError::Internal(format!("Failed to deserialize session: {}", e))
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
            .map_err(|e| MemoryStoreError::Internal(format!("Failed to serialize session: {}", e)))?;

        redis::cmd("SETEX")
            .arg(&key)
            .arg(self.ttl_seconds)
            .arg(&json)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| MemoryStoreError::Internal(format!("Redis SETEX failed: {}", e)))?;

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
            .map_err(|e| MemoryStoreError::Internal(format!("Redis DEL failed: {}", e)))?;

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
            .map_err(|e| MemoryStoreError::Internal(format!("Redis EXISTS failed: {}", e)))?;

        Ok(exists > 0)
    }

    async fn list_keys(&self) -> Result<Vec<String>> {
        let mut conn = self.get_connection().await?;
        let pattern = format!("{}*", self.prefix);

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| MemoryStoreError::Internal(format!("Redis KEYS failed: {}", e)))?;

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
// Redis tests require a running Redis instance
    // Run with: cargo test --features redis-tests
    #[cfg(feature = "redis-tests")]
    mod tests {
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
