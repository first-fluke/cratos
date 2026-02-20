
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DiscoveryConfig::default();
        assert!(config.enabled);
        assert_eq!(config.service_name, "Cratos AI");
        assert!(!config.instance_name.is_empty());
    }

    #[test]
    fn test_service_type() {
        assert_eq!(SERVICE_TYPE, "_cratos._tcp.local.");
    }

    #[test]
    fn test_discovery_service_new() {
        let config = DiscoveryConfig {
            enabled: false,
            service_name: "Test".to_string(),
            instance_name: "test-host".to_string(),
        };
        let svc = DiscoveryService::new(config);
        assert!(!svc.is_running());
    }
