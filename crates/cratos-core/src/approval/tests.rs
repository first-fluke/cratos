use super::*;
use uuid::Uuid;

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
async fn test_resolve_with_valid_nonce() {
    use crate::auth::{AuthContext, AuthMethod, Scope};
    let manager = ApprovalManager::new();

    let (request, _rx) = manager
        .create_request_async(
            Uuid::new_v4(),
            "telegram",
            "123",
            "user1",
            "Delete file",
            "Risky",
            None,
        )
        .await;

    let auth = AuthContext {
        user_id: "user1".to_string(),
        method: AuthMethod::ApiKey,
        scopes: vec![Scope::ApprovalRespond],
        session_id: None,
        device_id: None,
    };

    let result = manager
        .resolve(request.id, request.nonce, ApprovalStatus::Approved, &auth)
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().status, ApprovalStatus::Approved);
}

#[tokio::test]
async fn test_resolve_with_invalid_nonce() {
    use crate::auth::{AuthContext, AuthMethod, Scope};
    let manager = ApprovalManager::new();

    let (request, _rx) = manager
        .create_request_async(
            Uuid::new_v4(),
            "telegram",
            "123",
            "user1",
            "Delete file",
            "Risky",
            None,
        )
        .await;

    let auth = AuthContext {
        user_id: "user1".to_string(),
        method: AuthMethod::ApiKey,
        scopes: vec![Scope::ApprovalRespond],
        session_id: None,
        device_id: None,
    };

    // Wrong nonce → replay defense
    let result = manager
        .resolve(request.id, Uuid::new_v4(), ApprovalStatus::Approved, &auth)
        .await;
    assert_eq!(result.unwrap_err(), ApprovalError::InvalidNonce);
}

#[tokio::test]
async fn test_resolve_unauthorized_user() {
    use crate::auth::{AuthContext, AuthMethod, Scope};
    let manager = ApprovalManager::new();

    let (request, _rx) = manager
        .create_request_async(
            Uuid::new_v4(),
            "telegram",
            "123",
            "user1",
            "Delete file",
            "Risky",
            None,
        )
        .await;

    let other_user = AuthContext {
        user_id: "attacker".to_string(),
        method: AuthMethod::ApiKey,
        scopes: vec![Scope::ApprovalRespond],
        session_id: None,
        device_id: None,
    };

    let result = manager
        .resolve(
            request.id,
            request.nonce,
            ApprovalStatus::Approved,
            &other_user,
        )
        .await;
    assert_eq!(result.unwrap_err(), ApprovalError::Unauthorized);
}

#[tokio::test]
async fn test_resolve_admin_can_override() {
    use crate::auth::{AuthContext, AuthMethod, Scope};
    let manager = ApprovalManager::new();

    let (request, _rx) = manager
        .create_request_async(
            Uuid::new_v4(),
            "telegram",
            "123",
            "user1",
            "Delete file",
            "Risky",
            None,
        )
        .await;

    let admin = AuthContext {
        user_id: "admin".to_string(),
        method: AuthMethod::ApiKey,
        scopes: vec![Scope::Admin],
        session_id: None,
        device_id: None,
    };

    let result = manager
        .resolve(request.id, request.nonce, ApprovalStatus::Approved, &admin)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_wait_async_resolved() {
    use crate::auth::{AuthContext, AuthMethod, Scope};
    let manager = Arc::new(ApprovalManager::new());

    let (request, rx) = manager
        .create_request_async(
            Uuid::new_v4(),
            "telegram",
            "123",
            "user1",
            "Test",
            "Test",
            None,
        )
        .await;

    let mgr = manager.clone();
    let nonce = request.nonce;
    let req_id = request.id;
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let auth = AuthContext {
            user_id: "user1".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::ApprovalRespond],
            session_id: None,
            device_id: None,
        };
        let _ = mgr
            .resolve(req_id, nonce, ApprovalStatus::Approved, &auth)
            .await;
    });

    let result = ApprovalManager::wait_async(rx, std::time::Duration::from_secs(5)).await;
    assert_eq!(result, ApprovalStatus::Approved);
}

#[tokio::test]
async fn test_wait_async_timeout() {
    let manager = ApprovalManager::new();

    let (_request, rx) = manager
        .create_request_async(
            Uuid::new_v4(),
            "telegram",
            "123",
            "user1",
            "Test",
            "Test",
            None,
        )
        .await;

    // Very short timeout → should get Rejected
    let result = ApprovalManager::wait_async(rx, std::time::Duration::from_millis(10)).await;
    assert_eq!(result, ApprovalStatus::Rejected);
}

#[tokio::test]
async fn test_create_request_async_emits_event() {
    use crate::event_bus::EventBus;
    let manager = ApprovalManager::new();
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    let exec_id = Uuid::new_v4();
    let (request, _) = manager
        .create_request_async(
            exec_id,
            "telegram",
            "123",
            "user1",
            "Test",
            "Test",
            Some(&bus),
        )
        .await;

    let event = rx.try_recv().unwrap();
    match event {
        crate::event_bus::OrchestratorEvent::ApprovalRequired {
            execution_id,
            request_id,
        } => {
            assert_eq!(execution_id, exec_id);
            assert_eq!(request_id, request.id);
        }
        _ => panic!("expected ApprovalRequired event"),
    }
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
