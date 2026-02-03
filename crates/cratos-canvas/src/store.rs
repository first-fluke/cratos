//! Session Store
//!
//! This module provides persistent storage for canvas sessions using SQLite.

use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePool, Row};
use uuid::Uuid;

use crate::document::CanvasDocument;
use crate::session::CanvasSession;

/// SQLite-based session store
pub struct SessionStore {
    pool: SqlitePool,
}

impl SessionStore {
    /// Create a new session store with the given database pool
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the database schema
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS canvas_sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                document_json TEXT NOT NULL,
                execution_id TEXT,
                created_at TEXT NOT NULL,
                last_accessed_at TEXT NOT NULL,
                metadata_json TEXT DEFAULT '{}'
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON canvas_sessions(user_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_last_accessed ON canvas_sessions(last_accessed_at);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Save a session to the database
    pub async fn save_session(&self, session: &CanvasSession) -> Result<(), sqlx::Error> {
        let document_json = serde_json::to_string(&session.document).unwrap_or_default();
        let metadata_json = serde_json::to_string(&session.metadata).unwrap_or_default();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO canvas_sessions
            (id, user_id, document_json, execution_id, created_at, last_accessed_at, metadata_json)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(session.id.to_string())
        .bind(&session.user_id)
        .bind(&document_json)
        .bind(session.execution_id.map(|id| id.to_string()))
        .bind(session.created_at.to_rfc3339())
        .bind(session.last_accessed_at.to_rfc3339())
        .bind(&metadata_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load a session from the database
    pub async fn load_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<CanvasSession>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, document_json, execution_id, created_at, last_accessed_at, metadata_json
            FROM canvas_sessions
            WHERE id = ?
            "#,
        )
        .bind(session_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id: String = row.get("id");
                let user_id: String = row.get("user_id");
                let document_json: String = row.get("document_json");
                let execution_id: Option<String> = row.get("execution_id");
                let created_at: String = row.get("created_at");
                let last_accessed_at: String = row.get("last_accessed_at");
                let metadata_json: String = row.get("metadata_json");

                let document: CanvasDocument = serde_json::from_str(&document_json)
                    .unwrap_or_else(|_| CanvasDocument::new("Error"));
                let metadata: serde_json::Value =
                    serde_json::from_str(&metadata_json).unwrap_or_default();

                Ok(Some(CanvasSession {
                    id: Uuid::parse_str(&id).unwrap_or_default(),
                    user_id,
                    document,
                    execution_id: execution_id.and_then(|s| Uuid::parse_str(&s).ok()),
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    last_accessed_at: DateTime::parse_from_rfc3339(&last_accessed_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    metadata,
                }))
            }
            None => Ok(None),
        }
    }

    /// List sessions for a user
    pub async fn list_user_sessions(
        &self,
        user_id: &str,
    ) -> Result<Vec<SessionSummary>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, created_at, last_accessed_at,
                   json_extract(document_json, '$.title') as title
            FROM canvas_sessions
            WHERE user_id = ?
            ORDER BY last_accessed_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let sessions = rows
            .iter()
            .map(|row| {
                let id: String = row.get("id");
                let title: Option<String> = row.get("title");
                let created_at: String = row.get("created_at");
                let last_accessed_at: String = row.get("last_accessed_at");

                SessionSummary {
                    id: Uuid::parse_str(&id).unwrap_or_default(),
                    title: title.unwrap_or_else(|| "Untitled".to_string()),
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    last_accessed_at: DateTime::parse_from_rfc3339(&last_accessed_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                }
            })
            .collect();

        Ok(sessions)
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM canvas_sessions WHERE id = ?
            "#,
        )
        .bind(session_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete expired sessions
    pub async fn delete_expired_sessions(&self, max_idle_secs: i64) -> Result<usize, sqlx::Error> {
        let cutoff = Utc::now() - chrono::Duration::seconds(max_idle_secs);

        let result = sqlx::query(
            r#"
            DELETE FROM canvas_sessions
            WHERE last_accessed_at < ?
            "#,
        )
        .bind(cutoff.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as usize)
    }

    /// Update last accessed timestamp
    pub async fn touch_session(&self, session_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE canvas_sessions
            SET last_accessed_at = ?
            WHERE id = ?
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .bind(session_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

/// Summary of a session for listing
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Session ID
    pub id: Uuid,
    /// Document title
    pub title: String,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last accessed
    pub last_accessed_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_test_db() -> SessionStore {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();

        let store = SessionStore::new(pool);
        store.init().await.unwrap();
        store
    }

    #[tokio::test]
    async fn test_session_store_init() {
        let _store = setup_test_db().await;
        // If we get here, init succeeded
    }

    #[tokio::test]
    async fn test_session_save_and_load() {
        let store = setup_test_db().await;

        let doc = CanvasDocument::new("Test Document");
        let session = CanvasSession::new("user1", doc);
        let session_id = session.id;

        store.save_session(&session).await.unwrap();

        let loaded = store.load_session(session_id).await.unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, session_id);
        assert_eq!(loaded.user_id, "user1");
        assert_eq!(loaded.document.title, "Test Document");
    }

    #[tokio::test]
    async fn test_session_delete() {
        let store = setup_test_db().await;

        let doc = CanvasDocument::new("Test");
        let session = CanvasSession::new("user1", doc);
        let session_id = session.id;

        store.save_session(&session).await.unwrap();
        assert!(store.load_session(session_id).await.unwrap().is_some());

        let deleted = store.delete_session(session_id).await.unwrap();
        assert!(deleted);

        assert!(store.load_session(session_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list_user_sessions() {
        let store = setup_test_db().await;

        // Create sessions for different users
        let doc1 = CanvasDocument::new("User1 Doc1");
        let session1 = CanvasSession::new("user1", doc1);
        store.save_session(&session1).await.unwrap();

        let doc2 = CanvasDocument::new("User1 Doc2");
        let session2 = CanvasSession::new("user1", doc2);
        store.save_session(&session2).await.unwrap();

        let doc3 = CanvasDocument::new("User2 Doc");
        let session3 = CanvasSession::new("user2", doc3);
        store.save_session(&session3).await.unwrap();

        let user1_sessions = store.list_user_sessions("user1").await.unwrap();
        assert_eq!(user1_sessions.len(), 2);

        let user2_sessions = store.list_user_sessions("user2").await.unwrap();
        assert_eq!(user2_sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_touch_session() {
        let store = setup_test_db().await;

        let doc = CanvasDocument::new("Test");
        let session = CanvasSession::new("user1", doc);
        let session_id = session.id;

        store.save_session(&session).await.unwrap();

        let original = store.load_session(session_id).await.unwrap().unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        store.touch_session(session_id).await.unwrap();

        let updated = store.load_session(session_id).await.unwrap().unwrap();
        assert!(updated.last_accessed_at >= original.last_accessed_at);
    }
}
