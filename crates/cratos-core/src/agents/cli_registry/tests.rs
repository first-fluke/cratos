
    use super::*;

    #[test]
    fn test_cli_config() {
        let config = CliConfig::claude();
        assert_eq!(config.name, "claude");
        assert_eq!(config.command, "claude");
    }

    #[test]
    fn test_cli_registry() {
        let registry = CliRegistry::with_defaults();
        assert!(!registry.list().is_empty());
        assert!(registry.get("groq").is_some());
        assert!(registry.get("anthropic").is_some());
    }
