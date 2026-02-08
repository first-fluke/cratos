//! Session Lifecycle Manager
//!
//! Manages session CRUD, ownership verification, and Lane serialization
//! (sequential message queue per session).

use crate::auth::{AuthContext, Scope};
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Status of a managed session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// Session is idle, ready for input
    Idle,
    /// An execution is currently running
    Running,
    /// Session has been closed
    Closed,
}

/// A queued input waiting to be processed (Lane serialization).
#[derive(Debug, Clone)]
pub struct QueuedInput {
    /// Unique queue entry ID
    pub id: Uuid,
    /// The text input
    pub text: String,
    /// When it was queued
    pub queued_at: DateTime<Utc>,
}

/// Summary view of a session (for list endpoints).
#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    /// Session ID
    pub id: Uuid,
    /// Display name
    pub name: Option<String>,
    /// Current status
    pub status: SessionStatus,
    /// Number of pending messages in queue
    pub pending_count: usize,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_accessed_at: DateTime<Utc>,
}

/// A managed session with ownership and Lane queue.
#[derive(Debug)]
pub struct ManagedSession {
    /// Unique session ID
    pub id: Uuid,
    /// Owner user ID (security: all access checks use this)
    pub owner_user_id: String,
    /// Underlying session store key
    pub session_key: String,
    /// Display name
    pub display_name: Option<String>,
    /// Current status
    pub status: SessionStatus,
    /// Currently running execution ID
    pub active_execution: Option<Uuid>,
    /// Pending message queue (Lane serialization)
    pub pending_queue: VecDeque<QueuedInput>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last access timestamp
    pub last_accessed_at: DateTime<Utc>,
}

impl ManagedSession {
    fn new(owner: &str, name: Option<String>) -> Self {
        let id = Uuid::new_v4();
        let now = Utc::now();
        Self {
            id,
            owner_user_id: owner.to_string(),
            session_key: format!("managed:{}:{}", owner, id),
            display_name: name,
            status: SessionStatus::Idle,
            active_execution: None,
            pending_queue: VecDeque::new(),
            created_at: now,
            last_accessed_at: now,
        }
    }

    fn to_summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id,
            name: self.display_name.clone(),
            status: self.status,
            pending_count: self.pending_queue.len(),
            created_at: self.created_at,
            last_accessed_at: self.last_accessed_at,
        }
    }
}

/// Session Manager — manages session lifecycle with ownership verification.
///
/// **Security invariants**:
/// - All session access checks `owner_user_id == requester.user_id || Admin`
/// - `list_sessions()` only returns sessions owned by the requester
pub struct SessionManager {
    sessions: RwLock<HashMap<Uuid, ManagedSession>>,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new session owned by the requester.
    pub async fn create_session(
        &self,
        requester: &AuthContext,
        name: Option<String>,
    ) -> Result<SessionSummary> {
        let session = ManagedSession::new(&requester.user_id, name);
        let summary = session.to_summary();
        self.sessions.write().await.insert(session.id, session);
        Ok(summary)
    }

    /// List sessions owned by the requester.
    /// Admin sees all sessions.
    pub async fn list_sessions(&self, requester: &AuthContext) -> Vec<SessionSummary> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|s| {
                s.status != SessionStatus::Closed
                    && (requester.has_scope(&Scope::Admin)
                        || s.owner_user_id == requester.user_id)
            })
            .map(|s| s.to_summary())
            .collect()
    }

    /// Get a session by ID with ownership check.
    pub async fn get_session(
        &self,
        id: Uuid,
        requester: &AuthContext,
    ) -> Result<SessionSummary> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(&id).ok_or_else(|| {
            Error::NotFound(format!("Session {} not found", id))
        })?;
        self.check_ownership(session, requester)?;
        Ok(session.to_summary())
    }

    /// Send a message to a session (Lane serialization).
    ///
    /// If the session is idle, returns `Some(text)` to indicate the caller
    /// should start execution. If busy, queues the message and returns `None`.
    pub async fn send_message(
        &self,
        session_id: Uuid,
        text: &str,
        requester: &AuthContext,
    ) -> Result<Option<String>> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or_else(|| {
            Error::NotFound(format!("Session {} not found", session_id))
        })?;
        self.check_ownership(session, requester)?;

        if session.status == SessionStatus::Closed {
            return Err(Error::InvalidState("Session is closed".to_string()));
        }

        session.last_accessed_at = Utc::now();

        if session.status == SessionStatus::Idle {
            // Start execution immediately
            session.status = SessionStatus::Running;
            session.active_execution = Some(Uuid::new_v4());
            Ok(Some(text.to_string()))
        } else {
            // Queue for later (Lane serialization)
            session.pending_queue.push_back(QueuedInput {
                id: Uuid::new_v4(),
                text: text.to_string(),
                queued_at: Utc::now(),
            });
            Ok(None)
        }
    }

    /// Mark the current execution as complete and pop the next queued message.
    ///
    /// Returns the next queued message text if there is one.
    pub async fn execution_completed(&self, session_id: Uuid) -> Result<Option<String>> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or_else(|| {
            Error::NotFound(format!("Session {} not found", session_id))
        })?;

        session.active_execution = None;

        if let Some(next) = session.pending_queue.pop_front() {
            // Start next execution
            session.active_execution = Some(Uuid::new_v4());
            session.last_accessed_at = Utc::now();
            Ok(Some(next.text))
        } else {
            session.status = SessionStatus::Idle;
            Ok(None)
        }
    }

    /// Cancel the active execution for a session.
    pub async fn cancel_execution(
        &self,
        session_id: Uuid,
        requester: &AuthContext,
    ) -> Result<bool> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or_else(|| {
            Error::NotFound(format!("Session {} not found", session_id))
        })?;
        self.check_ownership(session, requester)?;

        if session.active_execution.is_some() {
            session.active_execution = None;
            session.status = SessionStatus::Idle;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Delete (close) a session.
    pub async fn delete_session(
        &self,
        session_id: Uuid,
        requester: &AuthContext,
    ) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or_else(|| {
            Error::NotFound(format!("Session {} not found", session_id))
        })?;
        self.check_ownership(session, requester)?;
        session.status = SessionStatus::Closed;
        Ok(())
    }

    /// Verify the requester owns the session or is Admin.
    fn check_ownership(
        &self,
        session: &ManagedSession,
        requester: &AuthContext,
    ) -> Result<()> {
        if requester.has_scope(&Scope::Admin) || session.owner_user_id == requester.user_id {
            Ok(())
        } else {
            Err(Error::Unauthorized(
                "Not authorized to access this session".to_string(),
            ))
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthMethod;

    fn user_auth(user_id: &str) -> AuthContext {
        AuthContext {
            user_id: user_id.to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![
                Scope::SessionRead,
                Scope::SessionWrite,
                Scope::ExecutionRead,
                Scope::ExecutionWrite,
            ],
            session_id: None,
            device_id: None,
        }
    }

    fn admin_auth() -> AuthContext {
        AuthContext {
            user_id: "admin".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::Admin],
            session_id: None,
            device_id: None,
        }
    }

    #[tokio::test]
    async fn test_create_session() {
        let mgr = SessionManager::new();
        let auth = user_auth("alice");
        let summary = mgr.create_session(&auth, Some("Test".to_string())).await.unwrap();

        assert_eq!(summary.name, Some("Test".to_string()));
        assert_eq!(summary.status, SessionStatus::Idle);
        assert_eq!(summary.pending_count, 0);
    }

    #[tokio::test]
    async fn test_list_sessions_ownership() {
        let mgr = SessionManager::new();
        let alice = user_auth("alice");
        let bob = user_auth("bob");

        mgr.create_session(&alice, Some("Alice's".to_string())).await.unwrap();
        mgr.create_session(&bob, Some("Bob's".to_string())).await.unwrap();

        // Alice sees only her sessions
        let alice_sessions = mgr.list_sessions(&alice).await;
        assert_eq!(alice_sessions.len(), 1);
        assert_eq!(alice_sessions[0].name, Some("Alice's".to_string()));

        // Bob sees only his
        let bob_sessions = mgr.list_sessions(&bob).await;
        assert_eq!(bob_sessions.len(), 1);
        assert_eq!(bob_sessions[0].name, Some("Bob's".to_string()));

        // Admin sees all
        let admin = admin_auth();
        let all_sessions = mgr.list_sessions(&admin).await;
        assert_eq!(all_sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_get_session_forbidden() {
        let mgr = SessionManager::new();
        let alice = user_auth("alice");
        let bob = user_auth("bob");

        let session = mgr.create_session(&alice, None).await.unwrap();

        // Bob cannot access Alice's session
        let result = mgr.get_session(session.id, &bob).await;
        assert!(result.is_err());

        // Admin can
        let admin = admin_auth();
        let result = mgr.get_session(session.id, &admin).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lane_serialization() {
        let mgr = SessionManager::new();
        let auth = user_auth("alice");
        let session = mgr.create_session(&auth, None).await.unwrap();

        // First message starts immediately
        let result = mgr.send_message(session.id, "msg1", &auth).await.unwrap();
        assert_eq!(result, Some("msg1".to_string()));

        // Second and third are queued (session is Running)
        let result = mgr.send_message(session.id, "msg2", &auth).await.unwrap();
        assert_eq!(result, None);
        let result = mgr.send_message(session.id, "msg3", &auth).await.unwrap();
        assert_eq!(result, None);

        // Check queue size
        let summary = mgr.get_session(session.id, &auth).await.unwrap();
        assert_eq!(summary.pending_count, 2);

        // Complete first execution → pops msg2
        let next = mgr.execution_completed(session.id).await.unwrap();
        assert_eq!(next, Some("msg2".to_string()));

        // Complete second → pops msg3
        let next = mgr.execution_completed(session.id).await.unwrap();
        assert_eq!(next, Some("msg3".to_string()));

        // Complete third → no more
        let next = mgr.execution_completed(session.id).await.unwrap();
        assert_eq!(next, None);

        // Session should be idle again
        let summary = mgr.get_session(session.id, &auth).await.unwrap();
        assert_eq!(summary.status, SessionStatus::Idle);
    }

    #[tokio::test]
    async fn test_cancel_execution() {
        let mgr = SessionManager::new();
        let auth = user_auth("alice");
        let session = mgr.create_session(&auth, None).await.unwrap();

        // Start execution
        mgr.send_message(session.id, "hello", &auth).await.unwrap();

        // Cancel
        let was_running = mgr.cancel_execution(session.id, &auth).await.unwrap();
        assert!(was_running);

        // Cancel again → not running
        let was_running = mgr.cancel_execution(session.id, &auth).await.unwrap();
        assert!(!was_running);
    }

    #[tokio::test]
    async fn test_delete_session() {
        let mgr = SessionManager::new();
        let auth = user_auth("alice");
        let session = mgr.create_session(&auth, None).await.unwrap();

        mgr.delete_session(session.id, &auth).await.unwrap();

        // Deleted sessions don't appear in list
        let sessions = mgr.list_sessions(&auth).await;
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_send_to_closed_session() {
        let mgr = SessionManager::new();
        let auth = user_auth("alice");
        let session = mgr.create_session(&auth, None).await.unwrap();

        mgr.delete_session(session.id, &auth).await.unwrap();

        let result = mgr.send_message(session.id, "hello", &auth).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_not_found() {
        let mgr = SessionManager::new();
        let auth = user_auth("alice");

        let result = mgr.get_session(Uuid::new_v4(), &auth).await;
        assert!(result.is_err());
    }
}
