
    use super::*;

    #[test]
    fn test_agent_config_new() {
        let agent = AgentConfig::new("test", "Test Agent", "A test agent");
        assert_eq!(agent.id, "test");
        assert!(agent.enabled);
    }

    #[test]
    fn test_agent_tool_config_permissive() {
        let config = AgentToolConfig::permissive();
        assert!(config.is_tool_allowed("any_tool"));
        assert!(config.is_tool_allowed("shell"));
    }

    #[test]
    fn test_agent_tool_config_restricted() {
        let config = AgentToolConfig::with_allowed(["read_file", "search"]);
        assert!(config.is_tool_allowed("read_file"));
        assert!(config.is_tool_allowed("search"));
        assert!(!config.is_tool_allowed("shell"));
    }

    #[test]
    fn test_agent_tool_config_deny_priority() {
        let mut config = AgentToolConfig::default();
        config.deny.insert("dangerous".to_string());
        assert!(!config.is_tool_allowed("dangerous"));
    }

    #[test]
    fn test_default_agents() {
        let agents = AgentConfig::defaults();
        assert!(!agents.is_empty());

        let backend = agents.iter().find(|a| a.id == "backend");
        assert!(backend.is_some());

        let frontend = agents.iter().find(|a| a.id == "frontend");
        assert!(frontend.is_some());
    }
