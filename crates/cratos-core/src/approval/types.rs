use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of an approval request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalStatus {
    /// Waiting for user decision
    Pending,
    /// User approved the action
    Approved,
    /// User rejected the action
    Rejected,
    /// Request expired without response
    Expired,
}

/// An approval request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique request ID
    pub id: Uuid,
    /// One-time nonce for replay defense — must match when resolving
    pub nonce: Uuid,
    /// Execution ID this belongs to
    pub execution_id: Uuid,
    /// Channel type
    pub channel_type: String,
    /// Channel ID
    pub channel_id: String,
    /// User ID who initiated the request
    pub user_id: String,
    /// User ID who responded (for audit)
    pub responder_id: Option<String>,
    /// Action description
    pub action: String,
    /// Tool name (if applicable)
    pub tool_name: Option<String>,
    /// Tool arguments (if applicable)
    pub tool_args: Option<serde_json::Value>,
    /// Risk level description
    pub risk_description: String,
    /// Current status
    pub status: ApprovalStatus,
    /// When the request was created
    pub created_at: DateTime<Utc>,
    /// When the request expires
    pub expires_at: DateTime<Utc>,
    /// When the user responded (if they did)
    pub responded_at: Option<DateTime<Utc>>,
}

impl ApprovalRequest {
    /// Create a new approval request
    #[must_use]
    pub fn new(
        execution_id: Uuid,
        channel_type: impl Into<String>,
        channel_id: impl Into<String>,
        user_id: impl Into<String>,
        action: impl Into<String>,
        risk_description: impl Into<String>,
        timeout_secs: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            nonce: Uuid::new_v4(),
            execution_id,
            channel_type: channel_type.into(),
            channel_id: channel_id.into(),
            user_id: user_id.into(),
            responder_id: None,
            action: action.into(),
            tool_name: None,
            tool_args: None,
            risk_description: risk_description.into(),
            status: ApprovalStatus::Pending,
            created_at: now,
            expires_at: now + Duration::seconds(timeout_secs),
            responded_at: None,
        }
    }

    /// Set the tool information
    #[must_use]
    pub fn with_tool(mut self, name: impl Into<String>, args: serde_json::Value) -> Self {
        self.tool_name = Some(name.into());
        self.tool_args = Some(args);
        self
    }

    /// Check if the request has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if the request is still pending
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.status == ApprovalStatus::Pending && !self.is_expired()
    }

    /// Check if a user is authorized to respond to this request
    ///
    /// By default, only the original requester can approve/reject.
    /// Override this for multi-user approval workflows.
    #[must_use]
    pub fn can_respond(&self, responder_user_id: &str) -> bool {
        // SECURITY: Only the original user can approve their own requests
        self.user_id == responder_user_id
    }

    /// Approve the request with responder verification
    ///
    /// Returns true if approved, false if not authorized or not pending
    pub fn approve_by(&mut self, responder_id: &str) -> bool {
        if !self.is_pending() {
            return false;
        }

        if !self.can_respond(responder_id) {
            return false;
        }

        self.status = ApprovalStatus::Approved;
        self.responder_id = Some(responder_id.to_string());
        self.responded_at = Some(Utc::now());
        true
    }

    /// Reject the request with responder verification
    ///
    /// Returns true if rejected, false if not authorized or not pending
    pub fn reject_by(&mut self, responder_id: &str) -> bool {
        if !self.is_pending() {
            return false;
        }

        if !self.can_respond(responder_id) {
            return false;
        }

        self.status = ApprovalStatus::Rejected;
        self.responder_id = Some(responder_id.to_string());
        self.responded_at = Some(Utc::now());
        true
    }

    /// Approve the request (without responder verification - for internal use)
    #[deprecated(note = "Use approve_by() with responder verification instead")]
    pub fn approve(&mut self) {
        if self.is_pending() {
            self.status = ApprovalStatus::Approved;
            self.responded_at = Some(Utc::now());
        }
    }

    /// Reject the request (without responder verification - for internal use)
    #[deprecated(note = "Use reject_by() with responder verification instead")]
    pub fn reject(&mut self) {
        if self.is_pending() {
            self.status = ApprovalStatus::Rejected;
            self.responded_at = Some(Utc::now());
        }
    }

    /// Mark as expired
    ///
    /// SECURITY: Expired requests are automatically rejected (fail-safe default)
    pub fn expire(&mut self) {
        if self.status == ApprovalStatus::Pending {
            // SECURITY: Expired = Rejected (fail-safe)
            // This ensures that unanswered requests don't accidentally allow actions
            self.status = ApprovalStatus::Rejected;
            self.responded_at = Some(Utc::now());
        }
    }

    /// Check if the request was effectively denied (rejected or expired)
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(
            self.status,
            ApprovalStatus::Rejected | ApprovalStatus::Expired
        )
    }
}

/// Error from approval resolution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    /// Request not found
    NotFound,
    /// Nonce doesn't match (replay attempt)
    InvalidNonce,
    /// Responder not authorized
    Unauthorized,
    /// Request already resolved or expired
    Expired,
}

impl std::fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "approval request not found"),
            Self::InvalidNonce => write!(f, "invalid nonce (possible replay)"),
            Self::Unauthorized => write!(f, "unauthorized responder"),
            Self::Expired => write!(f, "approval request expired"),
        }
    }
}

impl std::error::Error for ApprovalError {}
