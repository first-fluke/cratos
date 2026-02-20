
    use super::*;

    #[test]
    fn test_tool_permissions_default() {
        let perms = ToolPermissions::default();

        // Always allowed tools
        assert_eq!(perms.check("search"), PermissionStatus::Allowed);
        assert_eq!(perms.check("read_file"), PermissionStatus::Allowed);

        // Require confirmation tools
        assert_eq!(
            perms.check("write_file"),
            PermissionStatus::RequiresConfirmation
        );
        assert_eq!(perms.check("shell"), PermissionStatus::RequiresConfirmation);

        // Always denied tools
        assert!(perms.check("rm -rf").is_denied());
        assert!(perms.check("sudo").is_denied());

        // Unknown tools require confirmation
        assert_eq!(
            perms.check("unknown_tool"),
            PermissionStatus::RequiresConfirmation
        );
    }

    #[test]
    fn test_tool_permissions_deny_patterns() {
        let perms = ToolPermissions::default();

        // Tools containing denied patterns should be blocked
        assert!(perms.check("sudo rm").is_denied());
        assert!(perms.check("rm -rf /").is_denied());
    }

    #[test]
    fn test_tool_permissions_modification() {
        let mut perms = ToolPermissions::default();

        // Allow a previously denied tool
        perms.allow("shell");
        assert_eq!(perms.check("shell"), PermissionStatus::Allowed);

        // Deny a previously allowed tool
        perms.deny("search");
        assert!(perms.check("search").is_denied());
    }

    #[test]
    fn test_channel_permissions() {
        // Configure Slack with restricted tools using struct init
        let slack_config = {
            let mut config = ChannelToolConfig::with_allowed(["search", "read_file"]);
            config.deny.insert("shell".to_string());
            config
        };

        let channels = ChannelPermissions {
            slack: slack_config,
            ..Default::default()
        };

        // Telegram uses default (all allowed)
        assert!(channels.is_tool_allowed("telegram", "shell"));
        assert!(channels.is_tool_allowed("telegram", "write_file"));

        // Slack has restrictions
        assert!(channels.is_tool_allowed("slack", "search"));
        assert!(!channels.is_tool_allowed("slack", "shell"));
        assert!(!channels.is_tool_allowed("slack", "write_file"));
    }

    #[test]
    fn test_permission_manager() {
        let manager = PermissionManager::permissive();

        // Check tool in telegram (permissive channel)
        assert_eq!(
            manager.check("search", "telegram"),
            PermissionStatus::Allowed
        );
        assert_eq!(
            manager.check("write_file", "telegram"),
            PermissionStatus::RequiresConfirmation
        );
        assert!(manager.check("sudo", "telegram").is_denied());
    }

    #[test]
    fn test_permission_status() {
        assert!(PermissionStatus::Allowed.is_allowed());
        assert!(PermissionStatus::RequiresConfirmation.is_allowed());
        assert!(!PermissionStatus::Denied("reason".to_string()).is_allowed());

        assert!(!PermissionStatus::Allowed.requires_confirmation());
        assert!(PermissionStatus::RequiresConfirmation.requires_confirmation());
        assert!(!PermissionStatus::Denied("reason".to_string()).requires_confirmation());
    }

    #[test]
    fn test_strict_mode() {
        let manager = PermissionManager::strict();

        // In strict mode, fewer tools are allowed by default
        assert_eq!(manager.check("search", "cli"), PermissionStatus::Allowed);
        // read_file should require confirmation in strict mode
        assert_eq!(
            manager.check("read_file", "cli"),
            PermissionStatus::RequiresConfirmation
        );
    }
