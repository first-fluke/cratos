
    use super::*;

    #[test]
    fn test_all_pass() {
        let input = AuditInput {
            auth_enabled: true,
            rate_limit_enabled: true,
            rate_limit_rpm: 60,
            sandbox_available: true,
            sandbox_image: Some("ubuntu:latest".to_string()),
            blocked_paths: vec![
                "/etc".to_string(),
                "/root".to_string(),
                "/dev".to_string(),
                "/proc".to_string(),
                "/sys".to_string(),
            ],
            credential_backend: "keychain".to_string(),
            injection_protection: true,
            e2e_available: true,
            tool_policy_rules: 4,
        };
        let report = run_audit(&input);
        assert_eq!(report.status(), "pass");
        assert_eq!(report.summary.total, 8);
        assert_eq!(report.summary.pass, 8);
    }

    #[test]
    fn test_all_fail() {
        let input = AuditInput::default();
        let report = run_audit(&input);
        assert_eq!(report.status(), "fail");
        assert!(report.summary.warnings > 0);
    }

    #[test]
    fn test_auth_disabled_warning() {
        let input = AuditInput {
            auth_enabled: false,
            ..Default::default()
        };
        let report = run_audit(&input);
        let auth_finding = report
            .findings
            .iter()
            .find(|f| f.check_name == "auth_enabled")
            .unwrap();
        assert_eq!(auth_finding.severity, Severity::Warning);
    }

    #[test]
    fn test_high_rate_limit_info() {
        let input = AuditInput {
            rate_limit_enabled: true,
            rate_limit_rpm: 500,
            ..Default::default()
        };
        let report = run_audit(&input);
        let rl_finding = report
            .findings
            .iter()
            .find(|f| f.check_name == "rate_limit")
            .unwrap();
        assert_eq!(rl_finding.severity, Severity::Info);
    }

    #[test]
    fn test_missing_blocked_paths() {
        let input = AuditInput {
            blocked_paths: vec!["/etc".to_string()],
            ..Default::default()
        };
        let report = run_audit(&input);
        let path_finding = report
            .findings
            .iter()
            .find(|f| f.check_name == "sensitive_paths")
            .unwrap();
        assert_eq!(path_finding.severity, Severity::Warning);
    }

    #[test]
    fn test_report_serialization() {
        let input = AuditInput::default();
        let report = run_audit(&input);
        let json = serde_json::to_string_pretty(&report).unwrap();
        assert!(json.contains("\"check_name\""));
        assert!(json.contains("\"summary\""));
    }
