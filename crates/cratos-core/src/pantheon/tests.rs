
    use super::*;

    #[test]
    fn test_domain_display() {
        assert_eq!(Domain::Dev.as_str(), "DEV");
        assert_eq!(Domain::Pm.as_str(), "PM");
        assert_eq!(Domain::Orchestrator.as_str(), "ORCHESTRATOR");
    }

    #[test]
    fn test_persona_level_default() {
        let level = PersonaLevel::default();
        assert_eq!(level.level, 1);
        assert_eq!(level.title, "Mortal");
    }

    #[test]
    fn test_persona_level_supreme() {
        let level = PersonaLevel::supreme();
        assert_eq!(level.level, PersonaLevel::SUPREME_LEVEL);
        assert_eq!(level.title, "Supreme");
    }
