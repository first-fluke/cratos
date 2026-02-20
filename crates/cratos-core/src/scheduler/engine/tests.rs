
    use super::*;
    use tempfile::TempDir;

    struct TestContext {
        engine: SchedulerEngine,
        _store: Arc<SchedulerStore>,
        _dir: TempDir,
    }

    async fn create_test_context() -> TestContext {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test_scheduler.db");
        let store = Arc::new(SchedulerStore::from_path(&path).await.unwrap());
        let config = SchedulerConfig::new().with_check_interval(1);
        let engine = SchedulerEngine::new(store.clone(), config);
        TestContext {
            engine,
            _store: store,
            _dir: dir,
        }
    }

    #[tokio::test]
    async fn test_add_and_list_tasks() {
        let ctx = create_test_context().await;

        let task = ScheduledTask::new(
            "test_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("Test prompt"),
        );

        ctx.engine.add_task(task).await.unwrap();

        let tasks = ctx.engine.list_tasks().await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "test_task");
    }

    #[tokio::test]
    async fn test_enable_disable_task() {
        let ctx = create_test_context().await;

        let task = ScheduledTask::new(
            "toggle_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("Test"),
        );
        let task_id = task.id;

        ctx.engine.add_task(task).await.unwrap();

        ctx.engine.set_task_enabled(task_id, false).await.unwrap();
        let task = ctx.engine.get_task(task_id).await.unwrap();
        assert!(!task.enabled);

        ctx.engine.set_task_enabled(task_id, true).await.unwrap();
        let task = ctx.engine.get_task(task_id).await.unwrap();
        assert!(task.enabled);
    }

    #[tokio::test]
    async fn test_remove_task() {
        let ctx = create_test_context().await;

        let task = ScheduledTask::new(
            "remove_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("Test"),
        );
        let task_id = task.id;

        ctx.engine.add_task(task).await.unwrap();
        ctx.engine.remove_task(task_id).await.unwrap();

        let result = ctx.engine.get_task(task_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_calculate_next_run_interval() {
        let ctx = create_test_context().await;

        let trigger = TriggerType::interval(3600);
        let now = Utc::now();
        let next = ctx.engine.calculate_next_run(&trigger, now);

        assert!(next.is_some());
        let next = next.unwrap();
        assert!(next > now);
        assert!((next - now).num_seconds() == 3600);
    }

    #[tokio::test]
    async fn test_running_count() {
        let ctx = create_test_context().await;
        assert_eq!(ctx.engine.running_count().await, 0);
    }
