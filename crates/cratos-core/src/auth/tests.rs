
    use super::*;

    #[test]
    fn test_generate_and_validate_key() {
        let store = AuthStore::new(true);
        let (key, _hash) = store
            .generate_api_key("user1", default_user_scopes(), "test key")
            .unwrap();

        let ctx = store.validate_token(key.expose()).unwrap();
        assert_eq!(ctx.user_id, "user1");
        assert!(ctx.has_scope(&Scope::SessionRead));
        assert!(!ctx.has_scope(&Scope::Admin));
    }

    #[test]
    fn test_invalid_token() {
        let store = AuthStore::new(true);
        let result = store.validate_token("invalid_token");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_token() {
        let store = AuthStore::new(true);
        let result = store.validate_token("");
        assert!(matches!(result, Err(AuthError::MissingCredentials)));
    }

    #[test]
    fn test_revoke_key() {
        let store = AuthStore::new(true);
        let (key, hash) = store
            .generate_api_key("user1", default_user_scopes(), "test")
            .unwrap();

        // Should work before revocation
        assert!(store.validate_token(key.expose()).is_ok());

        // Revoke
        store.revoke_key(&hash).unwrap();

        // Should fail after revocation
        let result = store.validate_token(key.expose());
        assert!(matches!(result, Err(AuthError::TokenRevoked)));
    }

    #[test]
    fn test_disabled_auth() {
        let store = AuthStore::new(false);
        let ctx = store.validate_token("anything").unwrap();
        assert_eq!(ctx.user_id, "anonymous");
        assert!(ctx.has_scope(&Scope::Admin));
    }

    #[test]
    fn test_scope_check() {
        let ctx = AuthContext {
            user_id: "user1".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::SessionRead, Scope::ExecutionRead],
            session_id: None,
            device_id: None,
        };

        assert!(ctx.has_scope(&Scope::SessionRead));
        assert!(ctx.has_scope(&Scope::ExecutionRead));
        assert!(!ctx.has_scope(&Scope::Admin));
        assert!(!ctx.has_scope(&Scope::ConfigWrite));
    }

    #[test]
    fn test_admin_scope_grants_all() {
        let ctx = AuthContext {
            user_id: "admin".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::Admin],
            session_id: None,
            device_id: None,
        };

        assert!(ctx.has_scope(&Scope::SessionRead));
        assert!(ctx.has_scope(&Scope::ConfigWrite));
        assert!(ctx.has_scope(&Scope::NodeManage));
    }

    #[test]
    fn test_require_scope() {
        let ctx = AuthContext {
            user_id: "user1".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::SessionRead],
            session_id: None,
            device_id: None,
        };

        assert!(ctx.require_scope(&Scope::SessionRead).is_ok());
        assert!(ctx.require_scope(&Scope::ConfigWrite).is_err());
    }

    #[test]
    fn test_list_keys() {
        let store = AuthStore::new(true);
        store
            .generate_api_key("user1", default_user_scopes(), "key1")
            .unwrap();
        store
            .generate_api_key("user2", admin_scopes(), "key2")
            .unwrap();

        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_scheduler_scopes() {
        let ctx = AuthContext {
            user_id: "user1".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::SchedulerRead],
            session_id: None,
            device_id: None,
        };

        assert!(ctx.has_scope(&Scope::SchedulerRead));
        assert!(!ctx.has_scope(&Scope::SchedulerWrite));
        assert!(ctx.require_scope(&Scope::SchedulerRead).is_ok());
        assert!(ctx.require_scope(&Scope::SchedulerWrite).is_err());

        // Admin should have scheduler scopes
        let admin_ctx = AuthContext {
            user_id: "admin".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::Admin],
            session_id: None,
            device_id: None,
        };
        assert!(admin_ctx.has_scope(&Scope::SchedulerRead));
        assert!(admin_ctx.has_scope(&Scope::SchedulerWrite));
    }

    #[test]
    fn test_default_user_scopes_include_scheduler_read() {
        let scopes = default_user_scopes();
        assert!(scopes.contains(&Scope::SchedulerRead));
        assert!(!scopes.contains(&Scope::SchedulerWrite));
    }

    #[test]
    fn test_active_key_count() {
        let store = AuthStore::new(true);
        let (_, hash) = store
            .generate_api_key("user1", default_user_scopes(), "key1")
            .unwrap();
        store
            .generate_api_key("user2", admin_scopes(), "key2")
            .unwrap();

        assert_eq!(store.active_key_count(), 2);

        store.revoke_key(&hash).unwrap();
        assert_eq!(store.active_key_count(), 1);
    }
