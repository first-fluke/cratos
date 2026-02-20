
    use super::*;

    #[test]
    fn test_planner_config() {
        let config = PlannerConfig::new()
            .with_max_iterations(5)
            .with_temperature(0.5)
            .with_tools(false);

        assert_eq!(config.max_iterations, 5);
        assert_eq!(config.temperature, Some(0.5));
        assert!(!config.include_tools);
    }

    #[test]
    fn test_plan_response() {
        let response = PlanResponse {
            content: Some("Hello".to_string()),
            tool_calls: Vec::new(),
            is_final: true,
            finish_reason: Some("stop".to_string()),
            model: "test".to_string(),
        };

        assert!(response.is_text_only());
        assert!(!response.has_tool_calls());
    }

    #[test]
    fn test_build_tool_result_messages() {
        let calls = vec![ToolCall {
            id: "call_1".to_string(),
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
            thought_signature: None,
        }];
        let results = vec![serde_json::json!({"result": "ok"})];

        let messages = Planner::build_tool_result_messages(&calls, &results);
        assert_eq!(messages.len(), 1);
    }
