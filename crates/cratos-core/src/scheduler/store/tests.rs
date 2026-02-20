    use super::*;
    use crate::scheduler::triggers::TriggerType;
    use crate::scheduler::types::{ScheduledTask, TaskAction};
    use tempfile::TempDir;

    struct TestContext {
        store: SchedulerStore,
        _dir: TempDir,
    }

    async fn create_test_context() -> TestContext {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test_scheduler.db");
        let store = SchedulerStore::from_path(&path).await.unwrap();
        TestContext { store, _dir: dir }
    }

    #[tokio::test]
    async fn test_create_and_get_task() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task = ScheduledTask::new(
            "test_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("Hello"),
        );

        store.create_task(&task).await.unwrap();

        let retrieved = store.get_task(task.id).await.unwrap();
        assert_eq!(retrieved.name, "test_task");
        assert_eq!(retrieved.id, task.id);
    }

    #[tokio::test]
    async fn test_update_task() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let mut task = ScheduledTask::new(
            "update_test",
            TriggerType::interval(3600),
            TaskAction::natural_language("Hello"),
        );

        store.create_task(&task).await.unwrap();

        task.name = "updated_name".to_string();
        task.enabled = false;

        store.update_task(&task).await.unwrap();

        let retrieved = store.get_task(task.id).await.unwrap();
        assert_eq!(retrieved.name, "updated_name");
        assert!(!retrieved.enabled);
    }

    #[tokio::test]
    async fn test_delete_task() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task = ScheduledTask::new(
            "delete_test",
            TriggerType::interval(3600),
            TaskAction::natural_language("Hello"),
        );

        store.create_task(&task).await.unwrap();
        store.delete_task(task.id).await.unwrap();

        let result = store.get_task(task.id).await;
        assert!(matches!(result, Err(SchedulerError::TaskNotFound(_))));
    }

    #[tokio::test]
    async fn test_list_enabled_tasks() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task1 = ScheduledTask::new(
            "enabled_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("Hello"),
        );

        let mut task2 = ScheduledTask::new(
            "disabled_task",
            TriggerType::interval(3600),
            TaskAction::natural_language("World"),
        );
        task2.enabled = false;

        store.create_task(&task1).await.unwrap();
        store.create_task(&task2).await.unwrap();

        let enabled = store.list_enabled_tasks().await.unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "enabled_task");
    }

    #[tokio::test]
    async fn test_record_execution() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task = ScheduledTask::new(
            "exec_test",
            TriggerType::interval(3600),
            TaskAction::natural_language("Hello"),
        );

        store.create_task(&task).await.unwrap();

        let exec_id = store.record_execution_start(task.id, 1).await.unwrap();

        store
            .record_execution_complete(exec_id, "success", Some("Completed"))
            .await
            .unwrap();

        let executions = store.get_task_executions(task.id, 10).await.unwrap();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].status, "success");
        assert_eq!(executions[0].result, Some("Completed".to_string()));
    }

    #[test]
    fn test_task_action_natural_language() {
        let action = TaskAction::natural_language("Test prompt");
        match action {
            TaskAction::NaturalLanguage { prompt, channel } => {
                assert_eq!(prompt, "Test prompt");
                assert!(channel.is_none());
            }
            _ => panic!("Expected NaturalLanguage variant"),
        }
    }

    #[test]
    fn test_task_action_natural_language_with_channel() {
        let action = TaskAction::NaturalLanguage {
            prompt: "Test".to_string(),
            channel: Some("telegram".to_string()),
        };
        match action {
            TaskAction::NaturalLanguage { prompt, channel } => {
                assert_eq!(prompt, "Test");
                assert_eq!(channel, Some("telegram".to_string()));
            }
            _ => panic!("Expected NaturalLanguage variant"),
        }
    }

    #[test]
    fn test_task_action_notification() {
        let action = TaskAction::Notification {
            channel: "telegram".to_string(),
            channel_id: "123456789".to_string(),
            message: "Hello, World!".to_string(),
        };
        match action {
            TaskAction::Notification {
                channel,
                channel_id,
                message,
            } => {
                assert_eq!(channel, "telegram");
                assert_eq!(channel_id, "123456789");
                assert_eq!(message, "Hello, World!");
            }
            _ => panic!("Expected Notification variant"),
        }
    }

    #[test]
    fn test_task_action_webhook() {
        let action = TaskAction::Webhook {
            url: "https://example.com/webhook".to_string(),
            method: "POST".to_string(),
            headers: Some(serde_json::json!({"Content-Type": "application/json"})),
            body: Some(serde_json::json!({"key": "value"})),
        };
        match action {
            TaskAction::Webhook {
                url,
                method,
                headers,
                body,
            } => {
                assert_eq!(url, "https://example.com/webhook");
                assert_eq!(method, "POST");
                assert!(headers.is_some());
                assert!(body.is_some());
            }
            _ => panic!("Expected Webhook variant"),
        }
    }

    #[test]
    fn test_task_action_shell() {
        let action = TaskAction::Shell {
            command: "ls -la".to_string(),
            cwd: Some("/tmp".to_string()),
        };
        match action {
            TaskAction::Shell { command, cwd } => {
                assert_eq!(command, "ls -la");
                assert_eq!(cwd, Some("/tmp".to_string()));
            }
            _ => panic!("Expected Shell variant"),
        }
    }

    #[test]
    fn test_task_action_tool_call() {
        let action = TaskAction::ToolCall {
            tool: "web_search".to_string(),
            args: serde_json::json!({"query": "rust programming"}),
        };
        match action {
            TaskAction::ToolCall { tool, args } => {
                assert_eq!(tool, "web_search");
                assert_eq!(args["query"], "rust programming");
            }
            _ => panic!("Expected ToolCall variant"),
        }
    }

    #[test]
    fn test_task_action_serialization() {
        let action = TaskAction::Notification {
            channel: "slack".to_string(),
            channel_id: "C123".to_string(),
            message: "Test".to_string(),
        };

        let json = serde_json::to_string(&action).unwrap();
        let deserialized: TaskAction = serde_json::from_str(&json).unwrap();

        match deserialized {
            TaskAction::Notification { channel, .. } => {
                assert_eq!(channel, "slack");
            }
            _ => panic!("Deserialization failed"),
        }
    }

    #[test]
    fn test_webhook_method_variants() {
        for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
            let action = TaskAction::Webhook {
                url: "https://example.com".to_string(),
                method: method.to_string(),
                headers: None,
                body: None,
            };

            if let TaskAction::Webhook { method: m, .. } = action {
                assert_eq!(m, method);
            }
        }
    }

    #[tokio::test]
    async fn test_create_notification_task() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task = ScheduledTask::new(
            "notification_task",
            TriggerType::interval(3600),
            TaskAction::Notification {
                channel: "telegram".to_string(),
                channel_id: "12345".to_string(),
                message: "Scheduled notification".to_string(),
            },
        );

        store.create_task(&task).await.unwrap();

        let retrieved = store.get_task(task.id).await.unwrap();
        match retrieved.action {
            TaskAction::Notification { message, .. } => {
                assert_eq!(message, "Scheduled notification");
            }
            _ => panic!("Expected Notification action"),
        }
    }

    #[tokio::test]
    async fn test_create_webhook_task() {
        let ctx = create_test_context().await;
        let store = &ctx.store;

        let task = ScheduledTask::new(
            "webhook_task",
            TriggerType::interval(1800),
            TaskAction::Webhook {
                url: "https://api.example.com/hook".to_string(),
                method: "POST".to_string(),
                headers: Some(serde_json::json!({"Authorization": "Bearer token"})),
                body: Some(serde_json::json!({"event": "scheduled"})),
            },
        );

        store.create_task(&task).await.unwrap();

        let retrieved = store.get_task(task.id).await.unwrap();
        match retrieved.action {
            TaskAction::Webhook { url, method, .. } => {
                assert_eq!(url, "https://api.example.com/hook");
                assert_eq!(method, "POST");
            }
            _ => panic!("Expected Webhook action"),
        }
    }
