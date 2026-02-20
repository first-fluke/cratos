
    use super::*;
    use std::sync::Mutex;

    // Global lock for tests that modify environment variables
    // This prevents race conditions when tests run in parallel
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // Helper to ensure tests run in non-production mode
    fn ensure_non_production() {
        std::env::remove_var("CRATOS_ENV");
        std::env::remove_var("CRATOS_ALLOW_MEMORY_STORE_IN_PRODUCTION");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Lock is for test isolation, not shared state
    async fn test_memory_store() {
        let _lock = ENV_LOCK.lock().unwrap();
        ensure_non_production();
        let store = MemoryStore::try_new().unwrap();

        // Initially empty
        assert_eq!(store.count().await.unwrap(), 0);

        // Create and save session
        let mut session = SessionContext::new("test:key");
        session.add_user_message("Hello");
        store.save(&session).await.unwrap();

        // Verify saved
        assert!(store.exists("test:key").await.unwrap());
        assert_eq!(store.count().await.unwrap(), 1);

        // Retrieve
        let loaded = store.get("test:key").await.unwrap().unwrap();
        assert_eq!(loaded.message_count(), 1);

        // Delete
        assert!(store.delete("test:key").await.unwrap());
        assert!(!store.exists("test:key").await.unwrap());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Lock is for test isolation, not shared state
    async fn test_memory_store_get_or_create() {
        let _lock = ENV_LOCK.lock().unwrap();
        ensure_non_production();
        let store = MemoryStore::try_new().unwrap();

        // First call creates
        let session1 = store.get_or_create("new:key").await;
        assert_eq!(session1.session_key, "new:key");

        // Second call returns same
        let session2 = store.get_or_create("new:key").await;
        assert_eq!(session1.id, session2.id);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Lock is for test isolation, not shared state
    async fn test_memory_store_list_keys() {
        let _lock = ENV_LOCK.lock().unwrap();
        ensure_non_production();
        let store = MemoryStore::try_new().unwrap();

        store.save(&SessionContext::new("key1")).await.unwrap();
        store.save(&SessionContext::new("key2")).await.unwrap();
        store.save(&SessionContext::new("key3")).await.unwrap();

        let keys = store.list_keys().await.unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    // ========================================================================
    // Production Safety Tests
    // Note: These tests use new_unsafe() to avoid environment variable races
    // ========================================================================

    #[test]
    fn test_memory_store_new_unsafe_always_works() {
        // new_unsafe should always work regardless of environment
        let store = MemoryStore::new_unsafe();
        // In test environment, bypass flag is set because we used new_unsafe
        assert!(store.is_production_bypass());
    }

    #[test]
    fn test_production_checks() {
        let _lock = ENV_LOCK.lock().unwrap();

        // Test 1: Non-production mode
        ensure_non_production();
        assert!(!is_production());
        assert!(!is_production_bypass_enabled());

        let store = MemoryStore::try_new().unwrap();
        assert!(!store.is_production_bypass());
        assert!(store.is_production_safe());

        // Test 2: Production mode without bypass - should fail
        std::env::set_var("CRATOS_ENV", "production");
        std::env::remove_var("CRATOS_ALLOW_MEMORY_STORE_IN_PRODUCTION");
        assert!(is_production());
        assert!(!is_production_bypass_enabled());

        let result = MemoryStore::try_new();
        assert!(result.is_err());
        if let Err(MemoryStoreError::Internal(msg)) = result {
            assert!(msg.contains("not allowed in production"));
        }

        // Test 3: Production mode with bypass - should succeed
        std::env::set_var("CRATOS_ALLOW_MEMORY_STORE_IN_PRODUCTION", "1");
        assert!(is_production());
        assert!(is_production_bypass_enabled());

        let store = MemoryStore::try_new().unwrap();
        assert!(store.is_production_bypass());
        assert!(!store.is_production_safe()); // Still not safe, just bypassed

        // Clean up
        ensure_non_production();
    }

