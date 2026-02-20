
    use super::*;

    #[tokio::test]
    async fn test_send_and_receive() {
        let router = A2aRouter::new(100);

        let msg = A2aMessage::new("backend", "frontend", "session-1", "API ready");
        router.send(msg).await;

        // Recipient should see the message
        let msgs = router.receive("frontend").await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].from_agent, "backend");
        assert_eq!(msgs[0].content, "API ready");

        // Queue should be drained
        let msgs = router.receive("frontend").await;
        assert!(msgs.is_empty());
    }

    #[tokio::test]
    async fn test_peek_does_not_drain() {
        let router = A2aRouter::new(100);

        let msg = A2aMessage::new("backend", "frontend", "s1", "hello");
        router.send(msg).await;

        let peeked = router.peek("frontend").await;
        assert_eq!(peeked.len(), 1);

        // Peek again — still there
        let peeked = router.peek("frontend").await;
        assert_eq!(peeked.len(), 1);

        // Receive drains
        let received = router.receive("frontend").await;
        assert_eq!(received.len(), 1);
        assert!(router.peek("frontend").await.is_empty());
    }

    #[tokio::test]
    async fn test_session_history() {
        let router = A2aRouter::new(100);

        router
            .send(A2aMessage::new("backend", "frontend", "s1", "step 1"))
            .await;
        router
            .send(A2aMessage::new("frontend", "qa", "s1", "step 2"))
            .await;
        router
            .send(A2aMessage::new(
                "backend",
                "frontend",
                "s2",
                "other session",
            ))
            .await;

        let history = router.session_history("s1").await;
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].content, "step 1");
        assert_eq!(history[1].content, "step 2");

        // Other session
        let history2 = router.session_history("s2").await;
        assert_eq!(history2.len(), 1);
    }

    #[tokio::test]
    async fn test_history_limit() {
        let router = A2aRouter::new(3);

        for i in 0..5 {
            router
                .send(A2aMessage::new("a", "b", "s1", format!("msg {}", i)))
                .await;
        }

        let history = router.session_history("s1").await;
        assert_eq!(history.len(), 3);
        // Oldest messages should be dropped
        assert_eq!(history[0].content, "msg 2");
        assert_eq!(history[2].content, "msg 4");
    }

    #[tokio::test]
    async fn test_pending_count() {
        let router = A2aRouter::new(100);

        assert_eq!(router.pending_count("frontend").await, 0);

        router
            .send(A2aMessage::new("a", "frontend", "s1", "1"))
            .await;
        router
            .send(A2aMessage::new("b", "frontend", "s1", "2"))
            .await;

        assert_eq!(router.pending_count("frontend").await, 2);

        router.receive("frontend").await;
        assert_eq!(router.pending_count("frontend").await, 0);
    }

    #[tokio::test]
    async fn test_clear_session() {
        let router = A2aRouter::new(100);

        router.send(A2aMessage::new("a", "b", "s1", "msg1")).await;
        router.send(A2aMessage::new("a", "b", "s2", "msg2")).await;

        router.clear_session("s1").await;

        // s1 history cleared
        assert!(router.session_history("s1").await.is_empty());
        // s2 unaffected
        assert_eq!(router.session_history("s2").await.len(), 1);

        // s1 messages removed from queue (b had 2 messages, now only s2's)
        let msgs = router.receive("b").await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].session_id, "s2");
    }

    #[tokio::test]
    async fn test_message_summary() {
        let router = A2aRouter::new(100);

        let long_content = "x".repeat(300);
        router
            .send(A2aMessage::new("a", "b", "s1", long_content))
            .await;

        let summaries = router.session_history_summaries("s1").await;
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].preview.len() < 210); // 200 + "..."
        assert!(summaries[0].preview.ends_with("..."));
    }

    #[tokio::test]
    async fn test_message_with_metadata() {
        let msg = A2aMessage::new("a", "b", "s1", "hello")
            .with_metadata("priority", serde_json::json!("high"))
            .with_metadata("tags", serde_json::json!(["urgent"]));

        assert_eq!(msg.metadata["priority"], "high");
        assert_eq!(msg.metadata.len(), 2);
    }

    #[tokio::test]
    async fn test_multiple_agents_isolation() {
        let router = A2aRouter::new(100);

        router
            .send(A2aMessage::new("a", "frontend", "s1", "for frontend"))
            .await;
        router
            .send(A2aMessage::new("a", "backend", "s1", "for backend"))
            .await;

        let fe_msgs = router.receive("frontend").await;
        assert_eq!(fe_msgs.len(), 1);
        assert_eq!(fe_msgs[0].content, "for frontend");

        let be_msgs = router.receive("backend").await;
        assert_eq!(be_msgs.len(), 1);
        assert_eq!(be_msgs[0].content, "for backend");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let msg = A2aMessage::new("backend", "frontend", "s1", "test content");
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: A2aMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.from_agent, "backend");
        assert_eq!(parsed.to_agent, "frontend");
        assert_eq!(parsed.content, "test content");
    }
