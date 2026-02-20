
    use super::*;

    #[test]
    fn test_default_config() {
        let config = QueueConfig::default();
        assert_eq!(config.mode, QueueMode::Sequential);
        assert_eq!(config.max_concurrent, 3);
    }

    #[tokio::test]
    async fn test_sequential_queue() {
        let queue = ExecutionQueue::new(QueueConfig::default());
        assert_eq!(queue.available_slots(), 1);

        let permit = queue.acquire().await;
        assert_eq!(queue.available_slots(), 0);

        drop(permit);
        assert_eq!(queue.available_slots(), 1);
    }

    #[tokio::test]
    async fn test_concurrent_queue() {
        let config = QueueConfig {
            mode: QueueMode::Concurrent,
            max_concurrent: 3,
            ..Default::default()
        };
        let queue = ExecutionQueue::new(config);
        assert_eq!(queue.available_slots(), 3);

        let p1 = queue.acquire().await;
        let p2 = queue.acquire().await;
        assert_eq!(queue.available_slots(), 1);

        drop(p1);
        assert_eq!(queue.available_slots(), 2);
        drop(p2);
    }

    #[test]
    fn test_queue_mode_serialization() {
        let mode = QueueMode::Sequential;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"sequential\"");

        let mode: QueueMode = serde_json::from_str("\"concurrent\"").unwrap();
        assert_eq!(mode, QueueMode::Concurrent);
    }
