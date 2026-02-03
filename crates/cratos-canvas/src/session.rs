//! Canvas Session Management
//!
//! This module provides session management for canvas documents.
//! Each session represents an active editing context.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::document::CanvasDocument;

/// A canvas session representing an active editing context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasSession {
    /// Unique session identifier
    pub id: Uuid,

    /// User who owns this session
    pub user_id: String,

    /// The document being edited
    pub document: CanvasDocument,

    /// Execution ID this session is associated with (if any)
    pub execution_id: Option<Uuid>,

    /// When the session was created
    pub created_at: DateTime<Utc>,

    /// When the session was last accessed
    pub last_accessed_at: DateTime<Utc>,

    /// Session metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl CanvasSession {
    /// Create a new session for a document
    #[must_use]
    pub fn new(user_id: impl Into<String>, document: CanvasDocument) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            document,
            execution_id: None,
            created_at: now,
            last_accessed_at: now,
            metadata: serde_json::json!({}),
        }
    }

    /// Create a session with a specific ID
    #[must_use]
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Associate with an execution
    #[must_use]
    pub fn with_execution(mut self, execution_id: Uuid) -> Self {
        self.execution_id = Some(execution_id);
        self
    }

    /// Update last accessed timestamp
    pub fn touch(&mut self) {
        self.last_accessed_at = Utc::now();
    }

    /// Check if session is expired (not accessed for given duration)
    #[must_use]
    pub fn is_expired(&self, max_idle_secs: i64) -> bool {
        let idle_duration = Utc::now() - self.last_accessed_at;
        idle_duration.num_seconds() > max_idle_secs
    }
}

/// Session manager for handling multiple canvas sessions
pub struct CanvasSessionManager {
    /// Active sessions by ID
    sessions: Arc<RwLock<HashMap<Uuid, CanvasSession>>>,

    /// Sessions by user ID for quick lookup
    user_sessions: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,

    /// Maximum idle time before session expires (in seconds)
    max_idle_secs: i64,

    /// Maximum sessions per user
    max_sessions_per_user: usize,
}

impl CanvasSessionManager {
    /// Create a new session manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            user_sessions: Arc::new(RwLock::new(HashMap::new())),
            max_idle_secs: 3600, // 1 hour default
            max_sessions_per_user: 10,
        }
    }

    /// Configure maximum idle time
    #[must_use]
    pub fn with_max_idle_secs(mut self, secs: i64) -> Self {
        self.max_idle_secs = secs;
        self
    }

    /// Configure maximum sessions per user
    #[must_use]
    pub fn with_max_sessions_per_user(mut self, max: usize) -> Self {
        self.max_sessions_per_user = max;
        self
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        user_id: impl Into<String>,
        document: CanvasDocument,
    ) -> CanvasSession {
        let user_id = user_id.into();
        let session = CanvasSession::new(&user_id, document);
        let session_id = session.id;

        // Add to sessions
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id, session.clone());
        }

        // Track user session
        {
            let mut user_sessions = self.user_sessions.write().await;
            let sessions = user_sessions.entry(user_id.clone()).or_default();
            sessions.push(session_id);

            // Limit sessions per user
            if sessions.len() > self.max_sessions_per_user {
                // Remove oldest sessions
                let to_remove: Vec<_> = sessions
                    .drain(0..sessions.len() - self.max_sessions_per_user)
                    .collect();

                drop(user_sessions);

                // Remove from main session store
                let mut all_sessions = self.sessions.write().await;
                for id in to_remove {
                    all_sessions.remove(&id);
                }
            }
        }

        session
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: Uuid) -> Option<CanvasSession> {
        let sessions = self.sessions.read().await;
        sessions.get(&session_id).cloned()
    }

    /// Get a mutable session reference and update it
    pub async fn update_session<F, R>(&self, session_id: Uuid, f: F) -> Option<R>
    where
        F: FnOnce(&mut CanvasSession) -> R,
    {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.touch();
            Some(f(session))
        } else {
            None
        }
    }

    /// Get all sessions for a user
    pub async fn get_user_sessions(&self, user_id: &str) -> Vec<CanvasSession> {
        let user_sessions = self.user_sessions.read().await;
        let session_ids = user_sessions.get(user_id).cloned().unwrap_or_default();

        let sessions = self.sessions.read().await;
        session_ids
            .iter()
            .filter_map(|id| sessions.get(id).cloned())
            .collect()
    }

    /// Remove a session
    pub async fn remove_session(&self, session_id: Uuid) -> Option<CanvasSession> {
        let session = {
            let mut sessions = self.sessions.write().await;
            sessions.remove(&session_id)
        };

        if let Some(ref s) = session {
            let mut user_sessions = self.user_sessions.write().await;
            if let Some(sessions) = user_sessions.get_mut(&s.user_id) {
                sessions.retain(|&id| id != session_id);
            }
        }

        session
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired(&self) -> usize {
        let expired_ids: Vec<Uuid> = {
            let sessions = self.sessions.read().await;
            sessions
                .iter()
                .filter(|(_, s)| s.is_expired(self.max_idle_secs))
                .map(|(id, _)| *id)
                .collect()
        };

        let count = expired_ids.len();
        for id in expired_ids {
            self.remove_session(id).await;
        }

        count
    }

    /// Get total number of active sessions
    pub async fn session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// List all session IDs
    pub async fn list_session_ids(&self) -> Vec<Uuid> {
        let sessions = self.sessions.read().await;
        sessions.keys().copied().collect()
    }
}

impl Default for CanvasSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_session_creation() {
        let doc = CanvasDocument::new("Test");
        let session = CanvasSession::new("user1", doc);

        assert_eq!(session.user_id, "user1");
        assert!(session.execution_id.is_none());
    }

    #[test]
    fn test_canvas_session_with_execution() {
        let doc = CanvasDocument::new("Test");
        let exec_id = Uuid::new_v4();
        let session = CanvasSession::new("user1", doc).with_execution(exec_id);

        assert_eq!(session.execution_id, Some(exec_id));
    }

    #[test]
    fn test_canvas_session_expiry() {
        let doc = CanvasDocument::new("Test");
        let mut session = CanvasSession::new("user1", doc);

        // Fresh session should not be expired
        assert!(!session.is_expired(3600));

        // Manually set last_accessed_at to the past
        session.last_accessed_at = Utc::now() - chrono::Duration::seconds(7200);
        assert!(session.is_expired(3600));
    }

    #[tokio::test]
    async fn test_session_manager_create() {
        let manager = CanvasSessionManager::new();
        let doc = CanvasDocument::new("Test");
        let session = manager.create_session("user1", doc).await;

        assert_eq!(session.user_id, "user1");
        assert_eq!(manager.session_count().await, 1);
    }

    #[tokio::test]
    async fn test_session_manager_get_session() {
        let manager = CanvasSessionManager::new();
        let doc = CanvasDocument::new("Test");
        let session = manager.create_session("user1", doc).await;
        let session_id = session.id;

        let retrieved = manager.get_session(session_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, session_id);
    }

    #[tokio::test]
    async fn test_session_manager_remove_session() {
        let manager = CanvasSessionManager::new();
        let doc = CanvasDocument::new("Test");
        let session = manager.create_session("user1", doc).await;
        let session_id = session.id;

        assert_eq!(manager.session_count().await, 1);
        manager.remove_session(session_id).await;
        assert_eq!(manager.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_session_manager_user_sessions() {
        let manager = CanvasSessionManager::new();

        // Create multiple sessions for the same user
        manager
            .create_session("user1", CanvasDocument::new("Doc1"))
            .await;
        manager
            .create_session("user1", CanvasDocument::new("Doc2"))
            .await;
        manager
            .create_session("user2", CanvasDocument::new("Doc3"))
            .await;

        let user1_sessions = manager.get_user_sessions("user1").await;
        assert_eq!(user1_sessions.len(), 2);

        let user2_sessions = manager.get_user_sessions("user2").await;
        assert_eq!(user2_sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_session_manager_max_sessions_per_user() {
        let manager = CanvasSessionManager::new().with_max_sessions_per_user(2);

        // Create 3 sessions (should remove the oldest)
        let s1 = manager
            .create_session("user1", CanvasDocument::new("Doc1"))
            .await;
        manager
            .create_session("user1", CanvasDocument::new("Doc2"))
            .await;
        manager
            .create_session("user1", CanvasDocument::new("Doc3"))
            .await;

        let sessions = manager.get_user_sessions("user1").await;
        assert_eq!(sessions.len(), 2);

        // First session should have been removed
        assert!(manager.get_session(s1.id).await.is_none());
    }

    #[tokio::test]
    async fn test_session_manager_update() {
        let manager = CanvasSessionManager::new();
        let doc = CanvasDocument::new("Original");
        let session = manager.create_session("user1", doc).await;
        let session_id = session.id;

        // Update the document title
        manager
            .update_session(session_id, |s| {
                s.document.title = "Updated".to_string();
            })
            .await;

        let updated = manager.get_session(session_id).await.unwrap();
        assert_eq!(updated.document.title, "Updated");
    }
}
