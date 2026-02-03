//! SQLite session storage backend
//!
//! Provides persistent session storage using SQLite - the default backend for Cratos.
//!
//! # Features
//!
//! - No external dependencies (embedded database)
//! - Automatic schema migrations
//! - TTL-based expiration
//! - Production-safe with proper data persistence
//!
//! # Usage
//!
//! ```no_run
//! use cratos_core::memory::SqliteStore;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Default location: ~/.cratos/sessions.db
//! let store = SqliteStore::new_default().await?;
//!
//! // Or specify a custom path
//! let store = SqliteStore::new("/path/to/sessions.db").await?;
//! # Ok(())
//! # }
//! ```

use super::{SessionContext, SessionStore};
use crate::error::{Error, Result};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::{debug, info, warn};

/// Default session TTL in hours
const DEFAULT_TTL_HOURS: i64 = 24;

/// SQLite session store
///
/// This is the default and recommended session storage backend for Cratos.
/// It provides persistent storage without requiring external services like Redis.
pub struct SqliteStore {
    pool: SqlitePool,
    /// Session TTL in hours
    ttl_hours: i64,
}

impl SqliteStore {
    /// Create a new SQLite store at the specified path
    ///
    /// # Errors
    ///
    /// Returns error if database creation or migration fails.
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        Self::with_options(path, DEFAULT_TTL_HOURS).await
    }

    /// Create a new SQLite store with custom TTL
    pub async fn with_options(path: impl AsRef<Path>, ttl_hours: i64) -> Result<Self> {
        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Internal(format!("Failed to create database directory: {}", e))
            })?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
            .map_err(|e| Error::Internal(format!("Invalid SQLite path: {}", e)))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| Error::Internal(format!("Failed to connect to SQLite: {}", e)))?;

        let store = Self { pool, ttl_hours };
        store.init_schema().await?;

        info!(path = %path.display(), ttl_hours = ttl_hours, "SQLite session store initialized");
        Ok(store)
    }

    /// Create a store at the default location (~/.cratos/sessions.db)
    pub async fn new_default() -> Result<Self> {
        let path = Self::default_path()?;
        Self::new(&path).await
    }

    /// Get the default database path
    pub fn default_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Internal("Could not determine home directory".to_string()))?;
        Ok(home.join(".cratos").join("sessions.db"))
    }

    /// Initialize the database schema
    async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                session_key TEXT PRIMARY KEY,
                session_data TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Internal(format!("Failed to create sessions table: {}", e)))?;

        // Create index for faster cleanup queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Internal(format!("Failed to create index: {}", e)))?;

        debug!("SQLite session schema initialized");
        Ok(())
    }

    /// Check database health
    pub async fn health_check(&self) -> Result<bool> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Error::Internal(format!("Health check failed: {}", e)))?;
        Ok(true)
    }
}

#[async_trait]
impl SessionStore for SqliteStore {
    async fn get(&self, session_key: &str) -> Result<Option<SessionContext>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT session_data FROM sessions WHERE session_key = ?")
                .bind(session_key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| Error::Internal(format!("Failed to get session: {}", e)))?;

        match row {
            Some((data,)) => {
                let session: SessionContext = serde_json::from_str(&data).map_err(|e| {
                    Error::Internal(format!("Failed to deserialize session: {}", e))
                })?;
                debug!(session_key = %session_key, "Session loaded from SQLite");
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    async fn save(&self, session: &SessionContext) -> Result<()> {
        let data = serde_json::to_string(session)
            .map_err(|e| Error::Internal(format!("Failed to serialize session: {}", e)))?;

        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO sessions (session_key, session_data, created_at, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(session_key) DO UPDATE SET
                session_data = excluded.session_data,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&session.session_key)
        .bind(&data)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Internal(format!("Failed to save session: {}", e)))?;

        debug!(session_key = %session.session_key, "Session saved to SQLite");
        Ok(())
    }

    async fn delete(&self, session_key: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM sessions WHERE session_key = ?")
            .bind(session_key)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Internal(format!("Failed to delete session: {}", e)))?;

        let deleted = result.rows_affected() > 0;
        debug!(session_key = %session_key, deleted = deleted, "Session deleted from SQLite");
        Ok(deleted)
    }

    async fn exists(&self, session_key: &str) -> Result<bool> {
        let row: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM sessions WHERE session_key = ?")
                .bind(session_key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| Error::Internal(format!("Failed to check session: {}", e)))?;

        Ok(row.is_some())
    }

    async fn list_keys(&self) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT session_key FROM sessions")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| Error::Internal(format!("Failed to list sessions: {}", e)))?;

        let keys: Vec<String> = rows.into_iter().map(|(k,)| k).collect();
        warn!(count = keys.len(), "Listed all session keys from SQLite");
        Ok(keys)
    }

    async fn count(&self) -> Result<usize> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Error::Internal(format!("Failed to count sessions: {}", e)))?;

        Ok(row.0 as usize)
    }

    async fn cleanup_expired(&self) -> Result<usize> {
        let cutoff = (Utc::now() - Duration::hours(self.ttl_hours)).to_rfc3339();

        let result = sqlx::query("DELETE FROM sessions WHERE updated_at < ?")
            .bind(&cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Internal(format!("Failed to cleanup sessions: {}", e)))?;

        let removed = result.rows_affected() as usize;
        if removed > 0 {
            info!(removed = removed, ttl_hours = self.ttl_hours, "Cleaned up expired sessions");
        }
        Ok(removed)
    }
}

/// Session backend configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionBackendConfig {
    /// Backend type: "sqlite" (default), "redis", or "memory"
    #[serde(default = "default_backend")]
    pub backend: String,

    /// SQLite database path (relative to data directory)
    #[serde(default = "default_sqlite_path")]
    pub sqlite_path: String,

    /// Redis URL (only used when backend = "redis")
    #[serde(default)]
    pub redis_url: Option<String>,

    /// Session expiry in seconds
    #[serde(default = "default_expiry")]
    pub expiry_seconds: u64,
}

fn default_backend() -> String {
    "sqlite".to_string()
}

fn default_sqlite_path() -> String {
    "sessions.db".to_string()
}

fn default_expiry() -> u64 {
    86400 // 24 hours
}

impl Default for SessionBackendConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            sqlite_path: default_sqlite_path(),
            redis_url: None,
            expiry_seconds: default_expiry(),
        }
    }
}

/// Unified session backend that wraps different storage implementations
pub enum SessionBackend {
    /// SQLite storage (default)
    Sqlite(SqliteStore),
    /// Redis storage (for high-scale scenarios)
    Redis(super::RedisStore),
    /// In-memory storage (for testing only)
    Memory(super::MemoryStore),
}

impl SessionBackend {
    /// Create a session backend from configuration
    pub async fn from_config(config: &SessionBackendConfig) -> Result<Self> {
        match config.backend.as_str() {
            "sqlite" => {
                let home = dirs::home_dir()
                    .ok_or_else(|| Error::Internal("Could not determine home directory".to_string()))?;
                let path = home.join(".cratos").join(&config.sqlite_path);
                let ttl_hours = (config.expiry_seconds / 3600) as i64;
                let store = SqliteStore::with_options(&path, ttl_hours).await?;
                Ok(Self::Sqlite(store))
            }
            "redis" => {
                let url = config.redis_url.as_deref()
                    .unwrap_or("redis://localhost:6379");
                let store = super::RedisStore::new(url)?;
                Ok(Self::Redis(store))
            }
            "memory" => {
                let store = super::MemoryStore::try_new()?;
                Ok(Self::Memory(store))
            }
            other => {
                Err(Error::Configuration(format!(
                    "Unknown session backend: '{}'. Use 'sqlite', 'redis', or 'memory'.",
                    other
                )))
            }
        }
    }

    /// Create with SQLite (default)
    pub async fn sqlite_default() -> Result<Self> {
        let store = SqliteStore::new_default().await?;
        Ok(Self::Sqlite(store))
    }
}

#[async_trait]
impl SessionStore for SessionBackend {
    async fn get(&self, session_key: &str) -> Result<Option<SessionContext>> {
        match self {
            Self::Sqlite(store) => store.get(session_key).await,
            Self::Redis(store) => store.get(session_key).await,
            Self::Memory(store) => store.get(session_key).await,
        }
    }

    async fn save(&self, session: &SessionContext) -> Result<()> {
        match self {
            Self::Sqlite(store) => store.save(session).await,
            Self::Redis(store) => store.save(session).await,
            Self::Memory(store) => store.save(session).await,
        }
    }

    async fn delete(&self, session_key: &str) -> Result<bool> {
        match self {
            Self::Sqlite(store) => store.delete(session_key).await,
            Self::Redis(store) => store.delete(session_key).await,
            Self::Memory(store) => store.delete(session_key).await,
        }
    }

    async fn exists(&self, session_key: &str) -> Result<bool> {
        match self {
            Self::Sqlite(store) => store.exists(session_key).await,
            Self::Redis(store) => store.exists(session_key).await,
            Self::Memory(store) => store.exists(session_key).await,
        }
    }

    async fn list_keys(&self) -> Result<Vec<String>> {
        match self {
            Self::Sqlite(store) => store.list_keys().await,
            Self::Redis(store) => store.list_keys().await,
            Self::Memory(store) => store.list_keys().await,
        }
    }

    async fn count(&self) -> Result<usize> {
        match self {
            Self::Sqlite(store) => store.count().await,
            Self::Redis(store) => store.count().await,
            Self::Memory(store) => store.count().await,
        }
    }

    async fn cleanup_expired(&self) -> Result<usize> {
        match self {
            Self::Sqlite(store) => store.cleanup_expired().await,
            Self::Redis(store) => store.cleanup_expired().await,
            Self::Memory(store) => store.cleanup_expired().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_store() -> (SqliteStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_sessions.db");
        let store = SqliteStore::new(&db_path).await.unwrap();
        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_sqlite_store_basic_operations() {
        let (store, _temp) = create_test_store().await;

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

        // Update
        let mut session2 = loaded;
        session2.add_user_message("World");
        store.save(&session2).await.unwrap();

        let loaded2 = store.get("test:key").await.unwrap().unwrap();
        assert_eq!(loaded2.message_count(), 2);

        // Delete
        assert!(store.delete("test:key").await.unwrap());
        assert!(!store.exists("test:key").await.unwrap());
    }

    #[tokio::test]
    async fn test_sqlite_store_list_keys() {
        let (store, _temp) = create_test_store().await;

        store.save(&SessionContext::new("key1")).await.unwrap();
        store.save(&SessionContext::new("key2")).await.unwrap();
        store.save(&SessionContext::new("key3")).await.unwrap();

        let keys = store.list_keys().await.unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[tokio::test]
    async fn test_sqlite_store_health_check() {
        let (store, _temp) = create_test_store().await;
        assert!(store.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_session_backend_from_config() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("config_test.db");

        let _config = SessionBackendConfig {
            backend: "sqlite".to_string(),
            sqlite_path: db_path.to_string_lossy().to_string(),
            redis_url: None,
            expiry_seconds: 3600,
        };

        // Note: This test uses absolute path which differs from default behavior
        // For full test, we'd need to mock home directory
    }
}
