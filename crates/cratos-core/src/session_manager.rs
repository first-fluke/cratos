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
                    && (requester.has_scope(&Scope::Admin) || s.owner_user_id == requester.user_id)
            })
            .map(|s| s.to_summary())
            .collect()
    }

    /// Get a session by ID with ownership check.
    pub async fn get_session(&self, id: Uuid, requester: &AuthContext) -> Result<SessionSummary> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(&id)
            .ok_or_else(|| Error::NotFound(format!("Session {} not found", id)))?;
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
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| Error::NotFound(format!("Session {} not found", session_id)))?;
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
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| Error::NotFound(format!("Session {} not found", session_id)))?;

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
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| Error::NotFound(format!("Session {} not found", session_id)))?;
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
    pub async fn delete_session(&self, session_id: Uuid, requester: &AuthContext) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| Error::NotFound(format!("Session {} not found", session_id)))?;
        self.check_ownership(session, requester)?;
        session.status = SessionStatus::Closed;
        Ok(())
    }

    /// Verify the requester owns the session or is Admin.
    fn check_ownership(&self, session: &ManagedSession, requester: &AuthContext) -> Result<()> {
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
mod tests;

