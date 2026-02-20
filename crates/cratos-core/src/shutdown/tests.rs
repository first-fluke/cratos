
    use super::*;

    #[tokio::test]
    async fn test_shutdown_phases() {
        let controller = ShutdownController::new();
        assert_eq!(controller.phase(), ShutdownPhase::Running);
        assert!(controller.is_accepting_work());
        assert!(!controller.is_shutting_down());

        controller.shutdown().await;

        assert_eq!(controller.phase(), ShutdownPhase::Terminated);
        assert!(!controller.is_accepting_work());
        assert!(controller.is_shutting_down());
    }

    #[tokio::test]
    async fn test_task_guard() {
        let controller = ShutdownController::new();

        assert_eq!(controller.active_task_count(), 0);

        {
            let _guard1 = controller.register_task();
            let _guard2 = controller.register_task();
            assert_eq!(controller.active_task_count(), 2);
        }

        // Guards dropped, count should be 0
        assert_eq!(controller.active_task_count(), 0);
    }

    #[tokio::test]
    async fn test_task_guard_complete() {
        let controller = ShutdownController::new();

        let guard = controller.register_task();
        assert_eq!(controller.active_task_count(), 1);

        guard.complete();
        assert_eq!(controller.active_task_count(), 0);
    }

    #[tokio::test]
    async fn test_cancellation_propagation() {
        let controller = ShutdownController::new();
        let token = controller.token();

        assert!(!token.is_cancelled());

        controller.cancel_token.cancel();

        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_double_shutdown() {
        let controller = ShutdownController::new();

        // First shutdown
        let c1 = controller.clone();
        let handle1 = tokio::spawn(async move {
            Arc::new(c1).shutdown().await;
        });

        // Second shutdown (should be no-op)
        let c2 = controller.clone();
        let handle2 = tokio::spawn(async move {
            Arc::new(c2).shutdown().await;
        });

        let _ = tokio::join!(handle1, handle2);

        assert_eq!(controller.phase(), ShutdownPhase::Terminated);
    }

    #[test]
    fn test_force_shutdown() {
        let controller = ShutdownController::new();

        controller.force_shutdown();

        assert_eq!(controller.phase(), ShutdownPhase::Terminated);
        assert!(controller.cancel_token.is_cancelled());
    }
