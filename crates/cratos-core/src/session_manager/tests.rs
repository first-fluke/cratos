
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
        let summary = mgr
            .create_session(&auth, Some("Test".to_string()))
            .await
            .unwrap();

        assert_eq!(summary.name, Some("Test".to_string()));
        assert_eq!(summary.status, SessionStatus::Idle);
        assert_eq!(summary.pending_count, 0);
    }

    #[tokio::test]
    async fn test_list_sessions_ownership() {
        let mgr = SessionManager::new();
        let alice = user_auth("alice");
        let bob = user_auth("bob");

        mgr.create_session(&alice, Some("Alice's".to_string()))
            .await
            .unwrap();
        mgr.create_session(&bob, Some("Bob's".to_string()))
            .await
            .unwrap();

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
