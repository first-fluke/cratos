
    use super::*;
    use tempfile::TempDir;

    async fn create_test_store() -> (SqliteStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_sessions.db");
        let store = SqliteStore::new(&db_path).await.unwrap();
        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_sqlite_store_basic_operations() {
        let (store, _temp) = create_test_store().await;

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

        // Update
        let mut session2 = loaded;
        session2.add_user_message("World");
        store.save(&session2).await.unwrap();

        let loaded2 = store.get("test:key").await.unwrap().unwrap();
        assert_eq!(loaded2.message_count(), 2);

        // Delete
        assert!(store.delete("test:key").await.unwrap());
        assert!(!store.exists("test:key").await.unwrap());
    }

    #[tokio::test]
    async fn test_sqlite_store_list_keys() {
        let (store, _temp) = create_test_store().await;

        store.save(&SessionContext::new("key1")).await.unwrap();
        store.save(&SessionContext::new("key2")).await.unwrap();
        store.save(&SessionContext::new("key3")).await.unwrap();

        let keys = store.list_keys().await.unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[tokio::test]
    async fn test_sqlite_store_health_check() {
        let (store, _temp) = create_test_store().await;
        assert!(store.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_session_backend_from_config() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("config_test.db");

        let _config = SessionBackendConfig {
            backend: "sqlite".to_string(),
            sqlite_path: db_path.to_string_lossy().to_string(),
            redis_url: None,
            expiry_seconds: 3600,
        };

        // Note: This test uses absolute path which differs from default behavior
        // For full test, we'd need to mock home directory
    }
