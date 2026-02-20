use chrono::{Duration, Utc};
use std::collections::HashMap;
use tokio::sync::{oneshot, RwLock};
use uuid::Uuid;

use super::types::{ApprovalError, ApprovalRequest, ApprovalStatus};
use crate::auth::AuthContext;
use crate::event_bus::{EventBus, OrchestratorEvent};

/// Manager for approval requests
pub struct ApprovalManager {
    requests: RwLock<HashMap<Uuid, ApprovalRequest>>,
    /// oneshot senders keyed by request ID — resolvers notify waiters
    resolvers: RwLock<HashMap<Uuid, oneshot::Sender<ApprovalStatus>>>,
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
            resolvers: RwLock::new(HashMap::new()),
            default_timeout_secs: 300, // 5 minutes
        }
    }

    /// Create with custom timeout
    #[must_use]
    pub fn with_timeout_secs(timeout_secs: i64) -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
            resolvers: RwLock::new(HashMap::new()),
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

    /// Create a request with EventBus notification and oneshot-based resolution.
    ///
    /// Returns `(ApprovalRequest, Receiver)`. Use `wait_async()` on the receiver
    /// to await the user's decision without polling.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_request_async(
        &self,
        execution_id: Uuid,
        channel_type: impl Into<String>,
        channel_id: impl Into<String>,
        user_id: impl Into<String>,
        action: impl Into<String>,
        risk_description: impl Into<String>,
        event_bus: Option<&EventBus>,
    ) -> (ApprovalRequest, oneshot::Receiver<ApprovalStatus>) {
        let request = ApprovalRequest::new(
            execution_id,
            channel_type,
            channel_id,
            user_id,
            action,
            risk_description,
            self.default_timeout_secs,
        );

        let (tx, rx) = oneshot::channel();

        {
            let mut requests = self.requests.write().await;
            requests.insert(request.id, request.clone());
        }
        {
            let mut resolvers = self.resolvers.write().await;
            resolvers.insert(request.id, tx);
        }

        // Publish ApprovalRequired event
        if let Some(bus) = event_bus {
            bus.publish(OrchestratorEvent::ApprovalRequired {
                execution_id,
                request_id: request.id,
            });
        }

        (request, rx)
    }

    /// Resolve an approval request with nonce verification and ownership check.
    ///
    /// **Security checks**:
    /// 1. Request exists
    /// 2. Nonce matches (replay defense)
    /// 3. Responder is the owner or Admin
    /// 4. Request is still pending (not expired)
    pub async fn resolve(
        &self,
        request_id: Uuid,
        nonce: Uuid,
        decision: ApprovalStatus,
        responder: &AuthContext,
    ) -> std::result::Result<ApprovalRequest, ApprovalError> {
        let mut requests = self.requests.write().await;
        let request = requests
            .get_mut(&request_id)
            .ok_or(ApprovalError::NotFound)?;

        // Check 1: Nonce must match (replay defense)
        if request.nonce != nonce {
            return Err(ApprovalError::InvalidNonce);
        }

        // Check 2: Ownership (original user or Admin)
        if request.user_id != responder.user_id && !responder.has_scope(&crate::auth::Scope::Admin)
        {
            return Err(ApprovalError::Unauthorized);
        }

        // Check 3: Must still be pending
        if !request.is_pending() {
            return Err(ApprovalError::Expired);
        }

        // Apply the decision
        request.status = decision;
        request.responder_id = Some(responder.user_id.clone());
        request.responded_at = Some(Utc::now());

        let resolved = request.clone();
        drop(requests);

        // Notify the waiter via oneshot
        let mut resolvers = self.resolvers.write().await;
        if let Some(tx) = resolvers.remove(&request_id) {
            let _ = tx.send(decision);
        }

        Ok(resolved)
    }

    /// Wait for a request to be resolved via oneshot (no polling).
    ///
    /// Returns the decision, or `Rejected` on timeout.
    pub async fn wait_async(
        rx: oneshot::Receiver<ApprovalStatus>,
        timeout: std::time::Duration,
    ) -> ApprovalStatus {
        tokio::select! {
            result = rx => {
                result.unwrap_or(ApprovalStatus::Rejected)
            }
            _ = tokio::time::sleep(timeout) => {
                ApprovalStatus::Rejected
            }
        }
    }

    /// Get a request by ID
    pub async fn get(&self, id: Uuid) -> Option<ApprovalRequest> {
        let requests = self.requests.read().await;
        requests.get(&id).cloned()
    }

    /// Approve a request with responder verification
    ///
    /// Returns Some(request) if approved, None if not found or not authorized
    pub async fn approve_by(&self, id: Uuid, responder_id: &str) -> Option<ApprovalRequest> {
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
    pub async fn reject_by(&self, id: Uuid, responder_id: &str) -> Option<ApprovalRequest> {
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
