
    use super::*;

    #[test]
    fn test_dev_tool_display_name() {
        assert_eq!(DevTool::ClaudeCode.display_name(), "Claude Code");
        assert_eq!(DevTool::GeminiCli.display_name(), "Gemini CLI");
        assert_eq!(DevTool::Codex.display_name(), "Codex");
        assert_eq!(DevTool::Cursor.display_name(), "Cursor");
    }

    #[test]
    fn test_dev_session_serialization() {
        let session = DevSession {
            tool: DevTool::ClaudeCode,
            project_path: Some("my-project".to_string()),
            status: SessionStatus::Active,
            detected_at: Utc::now(),
            last_activity: Utc::now(),
            pid: Some(12345),
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("claude_code"));
        assert!(json.contains("my-project"));
        assert!(json.contains("12345"));
    }

    #[tokio::test]
    async fn test_monitor_sessions_empty() {
        let monitor = DevSessionMonitor::new(Duration::from_secs(60));
        let sessions = monitor.sessions().await;
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_monitor_poll() {
        let monitor = DevSessionMonitor::new(Duration::from_secs(60));
        monitor.poll_once().await;
        // Should not panic; results depend on running processes
        let sessions = monitor.sessions().await;
        // Just verify it returns without error
        let _ = sessions.len();
    }

    #[tokio::test]
    async fn test_sessions_for_tool() {
        let monitor = DevSessionMonitor::new(Duration::from_secs(60));
        let sessions = monitor.sessions_for_tool(DevTool::ClaudeCode).await;
        assert!(sessions.is_empty());
    }
