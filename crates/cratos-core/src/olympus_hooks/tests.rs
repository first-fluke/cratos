
    use super::*;

    #[test]
    fn test_olympus_config_default() {
        let config = OlympusConfig::default();
        assert!(config.auto_chronicle);
        assert!(config.auto_promotion);
        assert!(config.enforcer.enabled);
    }

    #[test]
    fn test_truncate_response() {
        assert_eq!(truncate_response("hello", 10), "hello");
        assert_eq!(truncate_response("hello world!", 5), "hello...");
        assert_eq!(truncate_response("line1\nline2\nline3", 100), "line1");
    }

    #[test]
    fn test_post_execution_summary_default() {
        let summary = PostExecutionSummary::default();
        assert!(!summary.chronicle_logged);
        assert!(!summary.promoted);
        assert!(summary.enforcement_actions.is_empty());
        assert!(summary.new_level.is_none());
    }
