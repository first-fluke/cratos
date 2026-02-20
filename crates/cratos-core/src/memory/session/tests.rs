
    use super::*;

    #[test]
    fn test_session_context() {
        let mut ctx = SessionContext::new("test:channel:user");
        assert_eq!(ctx.message_count(), 0);

        ctx.add_user_message("Hello");
        ctx.add_assistant_message("Hi there!");
        assert_eq!(ctx.message_count(), 2);

        let messages = ctx.get_messages();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_session_key() {
        let key = SessionContext::make_key("telegram", "123", "456");
        assert_eq!(key, "telegram:123:456");
    }

    #[test]
    fn test_trim_messages_legacy() {
        let mut ctx = SessionContext::legacy("test:key");
        ctx.max_context_size = 3;

        for i in 0..5 {
            ctx.add_user_message(format!("Message {}", i));
        }

        assert_eq!(ctx.message_count(), 3);
    }

    #[test]
    fn test_metadata() {
        let mut ctx = SessionContext::new("test:key");
        ctx.set_metadata("key1", serde_json::json!("value1"));

        assert_eq!(ctx.get_metadata("key1"), Some(&serde_json::json!("value1")));
        assert_eq!(ctx.get_metadata("nonexistent"), None);
    }

    // ========================================================================
    // Token-Aware Trimming Tests
    // ========================================================================

    #[test]
    fn test_token_count() {
        let mut ctx = SessionContext::new("test:key");
        assert_eq!(ctx.token_count(), 0);

        ctx.add_user_message("Hello, world!");
        assert!(ctx.token_count() > 0);
    }

    #[test]
    fn test_with_token_budget() {
        let ctx = SessionContext::with_token_budget("test:key", 50_000);
        assert_eq!(ctx.max_tokens, 50_000);
        assert!(ctx.token_aware_trimming);
    }

    #[test]
    fn test_remaining_tokens() {
        let mut ctx = SessionContext::with_token_budget("test:key", 1000);

        let initial = ctx.remaining_tokens();
        assert_eq!(initial, 1000);

        ctx.add_user_message("Hello!");
        assert!(ctx.remaining_tokens() < initial);
    }

    #[test]
    fn test_would_exceed_budget() {
        let ctx = SessionContext::with_token_budget("test:key", 100);

        // Small message should not exceed
        let small_msg = Message::user("Hi");
        assert!(!ctx.would_exceed_budget(&small_msg));

        // Large message should exceed
        let large_msg = Message::user("A".repeat(1000));
        assert!(ctx.would_exceed_budget(&large_msg));
    }

    #[test]
    fn test_token_aware_trimming_preserves_system() {
        let mut ctx = SessionContext::with_token_budget("test:key", 500);

        ctx.add_system_message("You are a helpful assistant.");
        ctx.add_user_message("Hello!");
        ctx.add_assistant_message("Hi there! How can I help?");

        // Add messages until we exceed budget
        for i in 0..20 {
            ctx.add_user_message(format!("Question {}: What is the meaning of life?", i));
            ctx.add_assistant_message(format!("Answer {}: 42", i));
        }

        // System message should be preserved
        let messages = ctx.get_messages();
        assert!(messages.iter().any(|m| m.role == MessageRole::System));
    }

    #[test]
    fn test_token_aware_trimming_prioritizes_importance() {
        let mut ctx = SessionContext::with_token_budget("test:key", 300);

        // Add messages of different types
        ctx.add_user_message("User message 1");
        ctx.add_assistant_message("Assistant response 1 - this is lower priority");
        ctx.add_user_message("User message 2");
        ctx.add_tool_message("Tool result - high priority", "tool_1");
        ctx.add_assistant_message("Assistant response 2 - this is lower priority");

        // Force trimming by adding more
        for _ in 0..10 {
            ctx.add_assistant_message("More assistant text to force trimming");
        }

        // Tool results should be more likely to survive than assistant messages
        let messages = ctx.get_messages();
        let has_tool = messages.iter().any(|m| m.role == MessageRole::Tool);
        let assistant_count = messages
            .iter()
            .filter(|m| m.role == MessageRole::Assistant)
            .count();

        // If there's limited space, tool messages should be preserved over assistant
        if messages.len() < 10 {
            // With tight budget, tool results should survive
            assert!(has_tool || assistant_count == 0);
        }
    }

    #[test]
    fn test_message_importance_ordering() {
        assert!(MessageImportance::System > MessageImportance::ToolResult);
        assert!(MessageImportance::ToolResult > MessageImportance::User);
        assert!(MessageImportance::User > MessageImportance::Assistant);
    }
