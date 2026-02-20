
    use super::*;

    /// Mock verifier for testing.
    struct MockVerifier {
        name: String,
        result: Result<ExternalAuthResult, ExternalAuthError>,
    }

    #[async_trait]
    impl ExternalAuthVerifier for MockVerifier {
        fn provider_name(&self) -> &str {
            &self.name
        }

        async fn verify(&self, _credential: &str) -> Result<ExternalAuthResult, ExternalAuthError> {
            self.result.clone()
        }
    }

    fn mock_success(name: &str) -> MockVerifier {
        MockVerifier {
            name: name.to_string(),
            result: Ok(ExternalAuthResult {
                user_id: "test-user".to_string(),
                display_name: Some("Test User".to_string()),
                scopes: vec![Scope::SessionRead, Scope::ExecutionRead],
                metadata: HashMap::new(),
            }),
        }
    }

    fn mock_failure(name: &str) -> MockVerifier {
        MockVerifier {
            name: name.to_string(),
            result: Err(ExternalAuthError::VerificationFailed(
                "invalid token".to_string(),
            )),
        }
    }

    #[tokio::test]
    async fn test_registry_register_and_verify() {
        let mut registry = ExternalAuthRegistry::new();
        registry.register(Box::new(mock_success("test-provider")));

        assert!(registry.has_provider("test-provider"));
        assert!(!registry.has_provider("other"));

        let result = registry.verify("test-provider", "any-credential").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().user_id, "test-user");
    }

    #[tokio::test]
    async fn test_registry_unknown_provider() {
        let registry = ExternalAuthRegistry::new();
        let result = registry.verify("unknown", "cred").await;
        assert!(matches!(result, Err(ExternalAuthError::Unavailable(_))));
    }

    #[tokio::test]
    async fn test_registry_verification_failure() {
        let mut registry = ExternalAuthRegistry::new();
        registry.register(Box::new(mock_failure("failing")));

        let result = registry.verify("failing", "cred").await;
        assert!(matches!(
            result,
            Err(ExternalAuthError::VerificationFailed(_))
        ));
    }

    #[tokio::test]
    async fn test_registry_providers_list() {
        let mut registry = ExternalAuthRegistry::new();
        registry.register(Box::new(mock_success("a")));
        registry.register(Box::new(mock_success("b")));

        let mut providers = registry.providers();
        providers.sort();
        assert_eq!(providers, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn test_tailscale_verifier_no_socket() {
        let verifier = TailscaleVerifier::with_socket("/nonexistent/path");
        let result = verifier.verify("100.64.1.1").await;
        assert!(matches!(result, Err(ExternalAuthError::Unavailable(_))));
    }
