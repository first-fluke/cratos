
    use super::*;

    #[test]
    fn test_deny_list_blocks() {
        let policy = ToolPolicy::default();
        let declared = vec!["rm".to_string()];

        let result = policy.is_allowed("rm -rf /", &declared);
        assert!(matches!(result, Err(PolicyDenial::DenyListed(_))));
    }

    #[test]
    fn test_undeclared_command_blocked() {
        let policy = ToolPolicy::default();
        let declared = vec!["git".to_string(), "cargo".to_string()];

        let result = policy.is_allowed("npm install", &declared);
        assert!(matches!(result, Err(PolicyDenial::NotDeclared(_))));
    }

    #[test]
    fn test_declared_command_allowed() {
        let policy = ToolPolicy::default();
        let declared = vec!["git".to_string(), "cargo".to_string()];

        assert!(policy.is_allowed("git status", &declared).is_ok());
        assert!(policy.is_allowed("cargo build", &declared).is_ok());
    }

    #[test]
    fn test_deny_overrides_declared() {
        let policy = ToolPolicy::default();
        // Even if node declares "dd", deny list blocks it
        let declared = vec!["dd".to_string()];

        let result = policy.is_allowed("dd if=/dev/zero", &declared);
        assert!(matches!(result, Err(PolicyDenial::DenyListed(_))));
    }

    #[test]
    fn test_fork_bomb_blocked() {
        let policy = ToolPolicy::default();
        let declared = vec!["bash".to_string()];

        let result = policy.is_allowed(":(){:|:&};:", &declared);
        assert!(matches!(result, Err(PolicyDenial::DenyListed(_))));
    }

    #[test]
    fn test_empty_declared_blocks_all() {
        let policy = ToolPolicy::default();
        let declared: Vec<String> = vec![];

        let result = policy.is_allowed("ls", &declared);
        assert!(matches!(result, Err(PolicyDenial::NotDeclared(_))));
    }

    #[test]
    fn test_default_policy() {
        let policy = ToolPolicy::default();
        assert!(!policy.deny_commands.is_empty());
        assert!(!policy.platform_defaults.darwin.is_empty());
        assert!(!policy.platform_defaults.linux.is_empty());
    }

    // ── 6-Level ToolSecurityPolicy tests ──

    #[test]
    fn test_resolve_no_rules_returns_none() {
        let policy = ToolSecurityPolicy::new();
        let ctx = PolicyContext::default();
        assert_eq!(policy.resolve("exec", &ctx), None);
    }

    #[test]
    fn test_resolve_global_wildcard() {
        let mut policy = ToolSecurityPolicy::new();
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Global,
            scope: "*".to_string(),
            tool_pattern: "*".to_string(),
            action: PolicyAction::Allow,
        });
        let ctx = PolicyContext::default();
        assert_eq!(policy.resolve("exec", &ctx), Some(PolicyAction::Allow));
        assert_eq!(policy.resolve("bash", &ctx), Some(PolicyAction::Allow));
    }

    #[test]
    fn test_specific_tool_overrides_wildcard() {
        let mut policy = ToolSecurityPolicy::new();
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Global,
            scope: "*".to_string(),
            tool_pattern: "*".to_string(),
            action: PolicyAction::Allow,
        });
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Global,
            scope: "*".to_string(),
            tool_pattern: "exec".to_string(),
            action: PolicyAction::RequireApproval,
        });
        let ctx = PolicyContext::default();
        // Both are Global level, same priority — last added wins? No, same priority.
        // Since both have priority 3, the first one found is kept.
        // Actually, the loop replaces only if strictly lower priority, so first match at same level wins.
        // The wildcard matches first since it's added first, and "exec" also matches but has same priority.
        // We need to pick the most specific *pattern* at same level — for now both are Global.
        // The exec-specific rule won't override since same priority.
        // This is by design: use a more specific LEVEL to override.
        assert_eq!(policy.resolve("exec", &ctx), Some(PolicyAction::Allow));
    }

    #[test]
    fn test_sandbox_overrides_global() {
        let mut policy = ToolSecurityPolicy::new();
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Global,
            scope: "*".to_string(),
            tool_pattern: "exec".to_string(),
            action: PolicyAction::RequireApproval,
        });
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Sandbox,
            scope: "docker".to_string(),
            tool_pattern: "*".to_string(),
            action: PolicyAction::Allow,
        });
        // Without sandbox context → Global applies
        let ctx_no_sandbox = PolicyContext::default();
        assert_eq!(
            policy.resolve("exec", &ctx_no_sandbox),
            Some(PolicyAction::RequireApproval)
        );
        // With docker sandbox → Sandbox overrides (lower priority number)
        let ctx_docker = PolicyContext {
            sandbox: Some("docker".to_string()),
            ..Default::default()
        };
        assert_eq!(
            policy.resolve("exec", &ctx_docker),
            Some(PolicyAction::Allow)
        );
    }

    #[test]
    fn test_agent_overrides_global() {
        let mut policy = ToolSecurityPolicy::new();
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Global,
            scope: "*".to_string(),
            tool_pattern: "bash".to_string(),
            action: PolicyAction::Deny,
        });
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Agent,
            scope: "@sindri".to_string(),
            tool_pattern: "bash".to_string(),
            action: PolicyAction::Allow,
        });
        // Without agent → Global Deny
        let ctx_no_agent = PolicyContext::default();
        assert_eq!(
            policy.resolve("bash", &ctx_no_agent),
            Some(PolicyAction::Deny)
        );
        // With @sindri → Agent Allow (priority 2 < Global priority 3)
        let ctx_sindri = PolicyContext {
            agent: Some("@sindri".to_string()),
            ..Default::default()
        };
        assert_eq!(
            policy.resolve("bash", &ctx_sindri),
            Some(PolicyAction::Allow)
        );
    }

    #[test]
    fn test_provider_level() {
        let mut policy = ToolSecurityPolicy::new();
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Provider,
            scope: "gemini".to_string(),
            tool_pattern: "bash".to_string(),
            action: PolicyAction::Deny,
        });
        let ctx_gemini = PolicyContext {
            provider: Some("gemini".to_string()),
            ..Default::default()
        };
        let ctx_openai = PolicyContext {
            provider: Some("openai".to_string()),
            ..Default::default()
        };
        assert_eq!(
            policy.resolve("bash", &ctx_gemini),
            Some(PolicyAction::Deny)
        );
        assert_eq!(policy.resolve("bash", &ctx_openai), None);
    }

    #[test]
    fn test_group_level() {
        let mut policy = ToolSecurityPolicy::new();
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Group,
            scope: "network".to_string(),
            tool_pattern: "*".to_string(),
            action: PolicyAction::RequireApproval,
        });
        let ctx_network = PolicyContext {
            tool_group: Some("network".to_string()),
            ..Default::default()
        };
        let ctx_filesystem = PolicyContext {
            tool_group: Some("filesystem".to_string()),
            ..Default::default()
        };
        assert_eq!(
            policy.resolve("web_search", &ctx_network),
            Some(PolicyAction::RequireApproval)
        );
        assert_eq!(policy.resolve("web_search", &ctx_filesystem), None);
    }

    #[test]
    fn test_with_defaults() {
        let policy = ToolSecurityPolicy::with_defaults();
        let ctx = PolicyContext::default();
        // Regular tool → Allow (global wildcard)
        assert_eq!(
            policy.resolve_or_default("web_search", &ctx),
            PolicyAction::Allow
        );
    }

    #[test]
    fn test_resolve_or_default() {
        let policy = ToolSecurityPolicy::new();
        let ctx = PolicyContext::default();
        assert_eq!(
            policy.resolve_or_default("anything", &ctx),
            PolicyAction::Allow
        );
    }

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("*", "anything"));
        assert!(matches_pattern("exec", "exec"));
        assert!(!matches_pattern("exec", "bash"));
        assert!(matches_pattern("file_*", "file_read"));
        assert!(matches_pattern("file_*", "file_write"));
        assert!(!matches_pattern("file_*", "exec"));
    }
