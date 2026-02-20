
    use super::*;

    fn default_enforcer() -> LawEnforcer {
        let config = EnforcerConfig::default();
        let store = ChronicleStore::with_path("/tmp/cratos-test-enforcer");
        LawEnforcer::new(config, store)
    }

    #[test]
    fn test_validate_good_response() {
        let enforcer = default_enforcer();
        let violations =
            enforcer.validate_response("sindri", "[DEV] Sindri Lv1 : Task completed", false);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_validate_missing_format() {
        let enforcer = default_enforcer();
        // Response with no role prefix AND no persona reference → violation
        let violations = enforcer.validate_response("sindri", "Task completed", false);
        assert!(!violations.is_empty());
        assert!(matches!(
            violations[0],
            LawViolation::MissingResponseFormat { .. }
        ));
    }

    #[test]
    fn test_validate_persona_ref_passes() {
        let enforcer = default_enforcer();
        // Response references persona name → no format violation
        let violations =
            enforcer.validate_response("sindri", "Sindri here. Task completed.", false);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_validate_disabled() {
        let config = EnforcerConfig {
            enabled: false,
            ..Default::default()
        };
        let store = ChronicleStore::with_path("/tmp/cratos-test-enforcer-disabled");
        let enforcer = LawEnforcer::new(config, store);
        let violations = enforcer.validate_response("sindri", "bad format", true);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_violation_description() {
        let violation = LawViolation::MissingResponseFormat {
            persona: "sindri".to_string(),
        };
        assert!(violation.description().contains("Art.6"));

        let violation = LawViolation::MissingCommitHash {
            persona: "sindri".to_string(),
        };
        assert!(violation.description().contains("Art.10"));
    }

    #[test]
    fn test_violation_article_ref() {
        assert_eq!(
            LawViolation::MissingResponseFormat {
                persona: "s".to_string()
            }
            .article_ref(),
            "6"
        );
        assert_eq!(
            LawViolation::MissingCommitHash {
                persona: "s".to_string()
            }
            .article_ref(),
            "10"
        );
    }

    #[test]
    fn test_enforcer_config_default() {
        let config = EnforcerConfig::default();
        assert!(config.enabled);
        assert!(!config.auto_silence);
        assert!(config.auto_judgment);
    }
