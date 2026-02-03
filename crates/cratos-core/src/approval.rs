//! Approval - User approval flow handling
//!
//! This module provides the approval system for high-risk operations.
//! When a tool or action requires user confirmation, this system
//! handles the approval workflow.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
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

/// Manager for approval requests
pub struct ApprovalManager {
    requests: RwLock<HashMap<Uuid, ApprovalRequest>>,
    /// Default timeout in seconds
    default_timeout_secs: i64,
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalManager {
    /// Create a new approval manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
            default_timeout_secs: 300, // 5 minutes
        }
    }

    /// Create with custom timeout
    #[must_use]
    pub fn with_timeout_secs(timeout_secs: i64) -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
            default_timeout_secs: timeout_secs,
        }
    }

    /// Create a new approval request
    pub async fn create_request(
        &self,
        execution_id: Uuid,
        channel_type: impl Into<String>,
        channel_id: impl Into<String>,
        user_id: impl Into<String>,
        action: impl Into<String>,
        risk_description: impl Into<String>,
    ) -> ApprovalRequest {
        let request = ApprovalRequest::new(
            execution_id,
            channel_type,
            channel_id,
            user_id,
            action,
            risk_description,
            self.default_timeout_secs,
        );

        let mut requests = self.requests.write().await;
        requests.insert(request.id, request.clone());

        request
    }

    /// Get a request by ID
    pub async fn get(&self, id: Uuid) -> Option<ApprovalRequest> {
        let requests = self.requests.read().await;
        requests.get(&id).cloned()
    }

    /// Approve a request with responder verification
    ///
    /// Returns Some(request) if approved, None if not found or not authorized
    pub async fn approve_by(
        &self,
        id: Uuid,
        responder_id: &str,
    ) -> Option<ApprovalRequest> {
        let mut requests = self.requests.write().await;
        if let Some(request) = requests.get_mut(&id) {
            if request.approve_by(responder_id) {
                Some(request.clone())
            } else {
                None // Not authorized or not pending
            }
        } else {
            None
        }
    }

    /// Reject a request with responder verification
    ///
    /// Returns Some(request) if rejected, None if not found or not authorized
    pub async fn reject_by(
        &self,
        id: Uuid,
        responder_id: &str,
    ) -> Option<ApprovalRequest> {
        let mut requests = self.requests.write().await;
        if let Some(request) = requests.get_mut(&id) {
            if request.reject_by(responder_id) {
                Some(request.clone())
            } else {
                None // Not authorized or not pending
            }
        } else {
            None
        }
    }

    /// Approve a request (deprecated - use approve_by for security)
    #[deprecated(note = "Use approve_by() with responder verification instead")]
    pub async fn approve(&self, id: Uuid) -> Option<ApprovalRequest> {
        let mut requests = self.requests.write().await;
        if let Some(request) = requests.get_mut(&id) {
            #[allow(deprecated)]
            request.approve();
            Some(request.clone())
        } else {
            None
        }
    }

    /// Reject a request (deprecated - use reject_by for security)
    #[deprecated(note = "Use reject_by() with responder verification instead")]
    pub async fn reject(&self, id: Uuid) -> Option<ApprovalRequest> {
        let mut requests = self.requests.write().await;
        if let Some(request) = requests.get_mut(&id) {
            #[allow(deprecated)]
            request.reject();
            Some(request.clone())
        } else {
            None
        }
    }

    /// Get all pending requests for a user
    pub async fn pending_for_user(&self, user_id: &str) -> Vec<ApprovalRequest> {
        let requests = self.requests.read().await;
        requests
            .values()
            .filter(|r| r.user_id == user_id && r.is_pending())
            .cloned()
            .collect()
    }

    /// Get all pending requests for an execution
    pub async fn pending_for_execution(&self, execution_id: Uuid) -> Vec<ApprovalRequest> {
        let requests = self.requests.read().await;
        requests
            .values()
            .filter(|r| r.execution_id == execution_id && r.is_pending())
            .cloned()
            .collect()
    }

    /// Clean up expired requests
    pub async fn cleanup_expired(&self) -> usize {
        let mut requests = self.requests.write().await;
        let initial_count = requests.len();

        // Mark expired requests
        for request in requests.values_mut() {
            if request.status == ApprovalStatus::Pending && request.is_expired() {
                request.expire();
            }
        }

        // Remove old requests (older than 1 hour regardless of status)
        let cutoff = Utc::now() - Duration::hours(1);
        requests.retain(|_, r| r.created_at > cutoff);

        initial_count - requests.len()
    }

    /// Wait for a request to be resolved
    pub async fn wait_for_resolution(
        &self,
        id: Uuid,
        poll_interval_ms: u64,
    ) -> Option<ApprovalRequest> {
        loop {
            let request = self.get(id).await?;

            match request.status {
                ApprovalStatus::Pending => {
                    if request.is_expired() {
                        // Mark as expired and return
                        let mut requests = self.requests.write().await;
                        if let Some(r) = requests.get_mut(&id) {
                            r.expire();
                            return Some(r.clone());
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(poll_interval_ms)).await;
                }
                _ => return Some(request),
            }
        }
    }
}

/// Trait for approval callbacks
#[async_trait::async_trait]
pub trait ApprovalCallback: Send + Sync {
    /// Called when an approval is needed
    async fn request_approval(&self, request: &ApprovalRequest) -> crate::Result<()>;

    /// Called when an approval is resolved
    async fn notify_resolution(&self, request: &ApprovalRequest) -> crate::Result<()>;
}

/// Shared approval manager type
pub type SharedApprovalManager = Arc<ApprovalManager>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_request() {
        let request = ApprovalRequest::new(
            Uuid::new_v4(),
            "telegram",
            "123",
            "456",
            "Delete file /tmp/test.txt",
            "This will permanently delete the file",
            60,
        );

        assert!(request.is_pending());
        assert_eq!(request.status, ApprovalStatus::Pending);
    }

    #[test]
    fn test_approval_approve_by_authorized_user() {
        let mut request = ApprovalRequest::new(
            Uuid::new_v4(),
            "telegram",
            "123",
            "456", // user_id
            "Test action",
            "Test risk",
            60,
        );

        // Same user can approve
        assert!(request.approve_by("456"));
        assert_eq!(request.status, ApprovalStatus::Approved);
        assert_eq!(request.responder_id, Some("456".to_string()));
        assert!(request.responded_at.is_some());
    }

    #[test]
    fn test_approval_reject_by_unauthorized_user() {
        let mut request = ApprovalRequest::new(
            Uuid::new_v4(),
            "telegram",
            "123",
            "456", // user_id
            "Test action",
            "Test risk",
            60,
        );

        // Different user cannot approve
        assert!(!request.approve_by("789"));
        assert_eq!(request.status, ApprovalStatus::Pending);
        assert!(request.responder_id.is_none());
    }

    #[test]
    fn test_approval_expire_becomes_rejected() {
        let mut request = ApprovalRequest::new(
            Uuid::new_v4(),
            "telegram",
            "123",
            "456",
            "Test action",
            "Test risk",
            60,
        );

        // SECURITY: Expired requests should be treated as rejected (fail-safe)
        request.expire();
        assert_eq!(request.status, ApprovalStatus::Rejected);
        assert!(request.is_denied());
    }

    #[test]
    fn test_is_denied() {
        let mut request = ApprovalRequest::new(
            Uuid::new_v4(),
            "telegram",
            "123",
            "456",
            "Test action",
            "Test risk",
            60,
        );

        assert!(!request.is_denied());

        request.reject_by("456");
        assert!(request.is_denied());
    }

    #[tokio::test]
    async fn test_approval_manager_with_verification() {
        let manager = ApprovalManager::new();

        let request = manager
            .create_request(
                Uuid::new_v4(),
                "telegram",
                "123",
                "456", // user_id
                "Test action",
                "Test risk",
            )
            .await;

        assert!(manager.get(request.id).await.is_some());

        let pending = manager.pending_for_user("456").await;
        assert_eq!(pending.len(), 1);

        // Should fail with wrong user
        let result = manager.approve_by(request.id, "789").await;
        assert!(result.is_none());

        // Should succeed with correct user
        let result = manager.approve_by(request.id, "456").await;
        assert!(result.is_some());

        let approved = manager.get(request.id).await.unwrap();
        assert_eq!(approved.status, ApprovalStatus::Approved);
        assert_eq!(approved.responder_id, Some("456".to_string()));
    }

    #[tokio::test]
    async fn test_approval_manager_cleanup_expired() {
        let manager = ApprovalManager::with_timeout_secs(1); // 1 second timeout

        let request = manager
            .create_request(
                Uuid::new_v4(),
                "telegram",
                "123",
                "456",
                "Test action",
                "Test risk",
            )
            .await;

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Cleanup should mark as rejected (not just expired)
        manager.cleanup_expired().await;

        let expired = manager.get(request.id).await.unwrap();
        assert_eq!(expired.status, ApprovalStatus::Rejected);
        assert!(expired.is_denied());
    }
}
