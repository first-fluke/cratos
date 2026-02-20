
    use super::*;

    #[test]
    fn test_working_memory() {
        let mut wm = WorkingMemory::new();

        wm.set("foo", serde_json::json!("bar"));
        assert_eq!(wm.get("foo"), Some(&serde_json::json!("bar")));

        wm.record_tool_execution(
            "test_tool",
            serde_json::json!({}),
            Some(serde_json::json!({"result": "ok"})),
            true,
            None,
        );
        assert_eq!(wm.tool_history.len(), 1);
        assert!(wm.last_tool_execution().unwrap().success);
    }

    #[test]
    fn test_working_memory_with_execution_id() {
        let id = Uuid::new_v4();
        let wm = WorkingMemory::with_execution_id(id);
        assert_eq!(wm.execution_id, Some(id));
    }

    #[test]
    fn test_clear() {
        let mut wm = WorkingMemory::new();
        wm.set("key", serde_json::json!("value"));
        wm.current_step = 5;
        wm.total_steps = 10;

        wm.clear();
        assert!(wm.variables.is_empty());
        assert_eq!(wm.current_step, 0);
        assert_eq!(wm.total_steps, 0);
    }
