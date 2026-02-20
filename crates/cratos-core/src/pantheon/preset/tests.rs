
    use super::*;

    fn create_test_preset() -> PersonaPreset {
        PersonaPreset {
            persona: PersonaInfo {
                name: "TestAgent".to_string(),
                title: "Test Title".to_string(),
                domain: Domain::Dev,
                description: Some("Test description".to_string()),
            },
            traits: PersonaTraits {
                core: "Test core trait".to_string(),
                philosophy: "Test philosophy".to_string(),
                communication_style: vec!["clarity".to_string(), "conciseness".to_string()],
            },
            principles: PersonaPrinciples {
                rules: [
                    ("1".to_string(), "First principle".to_string()),
                    ("2".to_string(), "Second principle".to_string()),
                ]
                .into_iter()
                .collect(),
            },
            skills: PersonaSkills {
                default: vec!["skill1".to_string(), "skill2".to_string()],
                acquired: vec!["skill3".to_string()],
            },
            level: PersonaLevel {
                level: 3,
                title: "Demigod".to_string(),
            },
            instructions: None,
        }
    }

    #[test]
    fn test_persona_level_display() {
        let normal = PersonaLevel {
            level: 5,
            title: "Titan".to_string(),
        };
        assert_eq!(normal.level_display(), "5");

        let supreme = PersonaLevel::supreme();
        assert_eq!(supreme.level_display(), "∞");
    }

    #[test]
    fn test_persona_level_is_supreme() {
        let normal = PersonaLevel::default();
        assert!(!normal.is_supreme());

        let supreme = PersonaLevel::supreme();
        assert!(supreme.is_supreme());
    }

    #[test]
    fn test_principles_sorted() {
        let mut principles = PersonaPrinciples::default();
        principles
            .rules
            .insert("3".to_string(), "Third".to_string());
        principles
            .rules
            .insert("1".to_string(), "First".to_string());
        principles
            .rules
            .insert("2".to_string(), "Second".to_string());

        let sorted = principles.sorted_rules();
        assert_eq!(*sorted[0].0, "1");
        assert_eq!(*sorted[1].0, "2");
        assert_eq!(*sorted[2].0, "3");
    }

    #[test]
    fn test_skills_all() {
        let skills = PersonaSkills {
            default: vec!["a".to_string(), "b".to_string()],
            acquired: vec!["c".to_string()],
        };
        assert_eq!(skills.all().len(), 3);
    }

    #[test]
    fn test_to_system_prompt() {
        let preset = create_test_preset();
        let prompt = preset.to_system_prompt("TestUser");

        assert!(prompt.contains("TestAgent"));
        assert!(prompt.contains("Test Title"));
        assert!(prompt.contains("Test core trait"));
        assert!(prompt.contains("Test philosophy"));
        assert!(prompt.contains("TestUser"));
        assert!(prompt.contains("Lv3"));
    }

    #[test]
    fn test_to_agent_config() {
        let preset = create_test_preset();
        let config = preset.to_agent_config("TestUser");

        assert_eq!(config.id, "testagent");
        assert_eq!(config.name, "TestAgent");
        assert!(config.enabled);
        assert_eq!(config.routing.priority, Domain::Dev.priority());
    }

    #[test]
    fn test_format_response() {
        let preset = create_test_preset();

        let response = preset.format_response("Task completed.", None);
        assert_eq!(response, "[TestAgent Lv3] Task completed.");

        let response_with_law = preset.format_response("Task completed.", Some("2"));
        assert_eq!(
            response_with_law,
            "[TestAgent Lv3] Per Laws Article 2, Task completed."
        );
    }

    #[test]
    fn test_preset_serialize_deserialize() {
        let preset = create_test_preset();
        let toml_str = toml::to_string(&preset).unwrap();
        let deserialized: PersonaPreset = toml::from_str(&toml_str).unwrap();

        assert_eq!(deserialized.persona.name, preset.persona.name);
        assert_eq!(deserialized.level.level, preset.level.level);
    }
