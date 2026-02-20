
    use super::*;

    #[tokio::test]
    async fn test_start_pairing_generates_6_digit_pin() {
        let mgr = PairingManager::new();
        let pin = mgr.start_pairing().await;
        assert_eq!(pin.len(), 6);
        assert!(pin.chars().all(|c| c.is_ascii_digit()));
    }

    #[tokio::test]
    async fn test_verify_pin_success() {
        let mgr = PairingManager::new();
        let pin = mgr.start_pairing().await;

        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin(&pin, "my-phone", vk.as_bytes()).await;

        assert!(result.success);
        assert!(result.device_id.is_some());
    }

    #[tokio::test]
    async fn test_verify_wrong_pin_fails() {
        let mgr = PairingManager::new();
        let _pin = mgr.start_pairing().await;

        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin("999999", "phone", vk.as_bytes()).await;

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_verify_expired_pin_fails() {
        let mgr = PairingManager {
            pin_ttl_secs: 0,
            ..Default::default()
        };
        let pin = mgr.start_pairing().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin(&pin, "phone", vk.as_bytes()).await;

        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("PIN expired"));
    }

    #[tokio::test]
    async fn test_list_and_unpair_devices() {
        let mgr = PairingManager::new();
        let pin = mgr.start_pairing().await;

        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin(&pin, "phone", vk.as_bytes()).await;
        let device_id = result.device_id.unwrap();

        let devices = mgr.list_devices().await;
        assert_eq!(devices.len(), 1);

        let removed = mgr.unpair_device(&device_id).await;
        assert!(removed);

        let devices = mgr.list_devices().await;
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn test_cleanup_sessions() {
        let mgr = PairingManager {
            pin_ttl_secs: 0,
            ..Default::default()
        };
        mgr.start_pairing().await;
        mgr.start_pairing().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let cleaned = mgr.cleanup_sessions().await;
        assert_eq!(cleaned, 2);
    }

    #[test]
    fn test_generate_pin_format() {
        for _ in 0..100 {
            let pin = generate_pin();
            assert_eq!(pin.len(), 6);
            let num: u32 = pin.parse().unwrap();
            assert!(num >= 100_000 && num < 1_000_000);
        }
    }

    #[tokio::test]
    async fn test_invalid_public_key_length() {
        let mgr = PairingManager::new();
        let pin = mgr.start_pairing().await;

        let result = mgr.verify_pin(&pin, "phone", &[0u8; 16]).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("32 bytes"));
    }

    #[tokio::test]
    async fn test_sqlite_persistence() {
        // Create in-memory SQLite for testing
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let mgr = PairingManager::new_with_db(pool.clone()).await.unwrap();

        let pin = mgr.start_pairing().await;
        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin(&pin, "test-device", vk.as_bytes()).await;
        assert!(result.success);

        let device_id = result.device_id.unwrap();

        // Verify device is in SQLite directly
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM paired_devices")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.0, 1);

        // Create a new manager from the same DB (simulates restart)
        let mgr2 = PairingManager::new_with_db(pool).await.unwrap();
        let devices = mgr2.list_devices().await;
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, device_id);
        assert_eq!(devices[0].device_name, "test-device");
    }

    #[tokio::test]
    async fn test_unpair_sqlite() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let mgr = PairingManager::new_with_db(pool.clone()).await.unwrap();

        let pin = mgr.start_pairing().await;
        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin(&pin, "phone", vk.as_bytes()).await;
        let device_id = result.device_id.unwrap();

        mgr.unpair_device(&device_id).await;

        // Verify device is gone from SQLite
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM paired_devices")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.0, 0);

        // Also gone from memory
        let devices = mgr.list_devices().await;
        assert!(devices.is_empty());
    }
