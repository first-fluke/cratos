
    use super::*;

    #[test]
    fn test_default_mapping() {
        let mapping = PersonaMapping::default_mapping();

        assert_eq!(mapping.to_agent_id("sindri"), Some("backend"));
        assert_eq!(mapping.to_agent_id("athena"), Some("pm"));
        assert_eq!(mapping.to_agent_id("heimdall"), Some("qa"));
        assert_eq!(mapping.to_agent_id("mimir"), Some("researcher"));
        assert_eq!(mapping.to_agent_id("cratos"), Some("orchestrator"));
    }

    #[test]
    fn test_reverse_mapping() {
        let mapping = PersonaMapping::default_mapping();

        // "backend" maps to "brok" (last inserted wins in HashMap)
        let backend_persona = mapping.to_persona_name("backend");
        assert!(backend_persona == Some("sindri") || backend_persona == Some("brok"));
        assert_eq!(mapping.to_persona_name("pm"), Some("athena"));
        assert_eq!(mapping.to_persona_name("qa"), Some("heimdall"));
        assert_eq!(mapping.to_persona_name("po"), Some("odin"));
        assert_eq!(mapping.to_persona_name("devops"), Some("thor"));
    }

    #[test]
    fn test_case_insensitive() {
        let mapping = PersonaMapping::default_mapping();

        assert_eq!(mapping.to_agent_id("SINDRI"), Some("backend"));
        assert_eq!(mapping.to_agent_id("Athena"), Some("pm"));
    }

    #[test]
    fn test_is_persona() {
        let mapping = PersonaMapping::default_mapping();

        assert!(mapping.is_persona("sindri"));
        assert!(mapping.is_persona("ATHENA"));
        assert!(!mapping.is_persona("unknown"));
    }

    #[test]
    fn test_persona_names() {
        let mapping = PersonaMapping::default_mapping();
        let names = mapping.persona_names();

        assert!(names.contains(&"sindri"));
        assert!(names.contains(&"athena"));
        assert!(names.contains(&"cratos"));
    }

    #[test]
    fn test_extract_persona_mention() {
        let mapping = PersonaMapping::default_mapping();

        let result = extract_persona_mention("@sindri implement the API", &mapping);
        assert!(result.is_some());
        let (agent_id, rest) = result.unwrap();
        assert_eq!(agent_id, "backend");
        assert_eq!(rest, "implement the API");
    }

    #[test]
    fn test_extract_persona_mention_no_match() {
        let mapping = PersonaMapping::default_mapping();

        // Unknown persona
        let result = extract_persona_mention("@unknown do something", &mapping);
        assert!(result.is_none());

        // Non-persona word
        let result = extract_persona_mention("hello world", &mapping);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_persona_bare_english() {
        let mapping = PersonaMapping::default_mapping();

        // Without @, bare English name
        let result = extract_persona_mention("sindri do something", &mapping);
        assert!(result.is_some());
        let (agent_id, rest) = result.unwrap();
        assert_eq!(agent_id, "backend");
        assert_eq!(rest, "do something");

        // With trailing comma
        let result = extract_persona_mention("nike, do the SNS automation", &mapping);
        assert!(result.is_some());
        let (agent_id, rest) = result.unwrap();
        assert_eq!(agent_id, "marketing");
        assert_eq!(rest, "do the SNS automation");
    }

    #[test]
    fn test_format_response_without_preset() {
        let mapping = PersonaMapping::default_mapping();

        let response = mapping.format_response("sindri", "task completed", None);
        // Default format since preset not loaded
        assert_eq!(response, "[sindri] task completed");
    }

    #[test]
    fn test_domain_to_agent_id() {
        assert_eq!(domain_to_agent_id(Domain::Dev), "backend");
        assert_eq!(domain_to_agent_id(Domain::Pm), "pm");
        assert_eq!(domain_to_agent_id(Domain::Qa), "qa");
        assert_eq!(domain_to_agent_id(Domain::Researcher), "researcher");
        assert_eq!(domain_to_agent_id(Domain::Orchestrator), "orchestrator");
    }

    // ── Multi-Persona Extraction Tests ──────────────────────────────────

    #[test]
    fn test_extract_all_parallel_single() {
        let mapping = PersonaMapping::default_mapping();

        let result = extract_all_persona_mentions("@sindri implement the API", &mapping);
        assert!(result.is_some());
        let extraction = result.unwrap();
        assert_eq!(extraction.mode, ExecutionMode::Parallel);
        assert_eq!(extraction.personas.len(), 1);
        assert_eq!(extraction.personas[0].name, "sindri");
        assert_eq!(extraction.personas[0].agent_id, "backend");
        assert_eq!(extraction.rest, "implement the API");
    }

    #[test]
    fn test_extract_all_parallel_multiple() {
        let mapping = PersonaMapping::default_mapping();

        // Multiple personas: @nike @apollo
        let result =
            extract_all_persona_mentions("@nike @apollo SNS 마케팅 전략 수립해줘", &mapping);
        assert!(result.is_some());
        let extraction = result.unwrap();
        assert_eq!(extraction.mode, ExecutionMode::Parallel);
        assert_eq!(extraction.personas.len(), 2);
        assert_eq!(extraction.personas[0].name, "nike");
        assert_eq!(extraction.personas[0].agent_id, "marketing");
        assert_eq!(extraction.personas[1].name, "apollo");
        assert_eq!(extraction.personas[1].agent_id, "ux");
        assert_eq!(extraction.rest, "SNS 마케팅 전략 수립해줘");
    }

    #[test]
    fn test_extract_all_parallel_bare_names() {
        let mapping = PersonaMapping::default_mapping();

        // Without @ prefix
        let result = extract_all_persona_mentions("nike apollo 작업해줘", &mapping);
        assert!(result.is_some());
        let extraction = result.unwrap();
        assert_eq!(extraction.personas.len(), 2);
        assert_eq!(extraction.personas[0].name, "nike");
        assert_eq!(extraction.personas[1].name, "apollo");
        assert_eq!(extraction.rest, "작업해줘");
    }

    #[test]
    fn test_extract_all_parallel_with_comma() {
        let mapping = PersonaMapping::default_mapping();

        // With comma separator
        let result = extract_all_persona_mentions("nike, apollo, 마케팅 해줘", &mapping);
        assert!(result.is_some());
        let extraction = result.unwrap();
        assert_eq!(extraction.personas.len(), 2);
        assert_eq!(extraction.rest, "마케팅 해줘");
    }

    #[test]
    fn test_extract_all_pipeline() {
        let mapping = PersonaMapping::default_mapping();

        let result =
            extract_all_persona_mentions("@athena 요구사항 분석 -> @sindri 구현", &mapping);
        assert!(result.is_some());
        let extraction = result.unwrap();
        assert_eq!(extraction.mode, ExecutionMode::Pipeline);
        assert_eq!(extraction.personas.len(), 2);
        assert_eq!(extraction.personas[0].name, "athena");
        assert_eq!(
            extraction.personas[0].instruction,
            Some("요구사항 분석".to_string())
        );
        assert_eq!(extraction.personas[1].name, "sindri");
        assert_eq!(extraction.personas[1].instruction, Some("구현".to_string()));
    }

    #[test]
    fn test_extract_all_pipeline_three_stages() {
        let mapping = PersonaMapping::default_mapping();

        let result = extract_all_persona_mentions(
            "@athena 계획 -> @sindri 구현 -> @heimdall 검증",
            &mapping,
        );
        assert!(result.is_some());
        let extraction = result.unwrap();
        assert_eq!(extraction.mode, ExecutionMode::Pipeline);
        assert_eq!(extraction.personas.len(), 3);
        assert_eq!(extraction.personas[0].name, "athena");
        assert_eq!(extraction.personas[1].name, "sindri");
        assert_eq!(extraction.personas[2].name, "heimdall");
    }

    #[test]
    fn test_extract_all_collaborative() {
        let mapping = PersonaMapping::default_mapping();

        let result = extract_all_persona_mentions(
            "@sindri @heimdall collaborate: API 개발하고 테스트",
            &mapping,
        );
        assert!(result.is_some());
        let extraction = result.unwrap();
        assert_eq!(extraction.mode, ExecutionMode::Collaborative);
        assert_eq!(extraction.personas.len(), 2);
        assert_eq!(extraction.personas[0].name, "sindri");
        assert_eq!(extraction.personas[1].name, "heimdall");
        assert_eq!(extraction.rest, "API 개발하고 테스트");
    }

    #[test]
    fn test_extract_all_collaborative_korean() {
        let mapping = PersonaMapping::default_mapping();

        let result =
            extract_all_persona_mentions("@sindri @heimdall 협업: 코드 작성하고 리뷰", &mapping);
        assert!(result.is_some());
        let extraction = result.unwrap();
        assert_eq!(extraction.mode, ExecutionMode::Collaborative);
        assert_eq!(extraction.personas.len(), 2);
        assert_eq!(extraction.rest, "코드 작성하고 리뷰");
    }

    #[test]
    fn test_extract_all_no_match() {
        let mapping = PersonaMapping::default_mapping();

        // No valid personas
        let result = extract_all_persona_mentions("hello world", &mapping);
        assert!(result.is_none());

        // Unknown persona
        let result = extract_all_persona_mentions("@unknown do something", &mapping);
        assert!(result.is_none());
    }

    #[test]
    fn test_backward_compat_single_persona() {
        let mapping = PersonaMapping::default_mapping();

        // Old function should still work with new implementation
        let result = extract_persona_mention("@sindri implement", &mapping);
        assert!(result.is_some());
        let (agent_id, rest) = result.unwrap();
        assert_eq!(agent_id, "backend");
        assert_eq!(rest, "implement");
    }

    #[test]
    fn test_backward_compat_multi_returns_first() {
        let mapping = PersonaMapping::default_mapping();

        // Old function returns first persona's agent_id
        // New implementation: extract_all parses ALL personas, rest is after all of them
        let result = extract_persona_mention("@nike @apollo 작업", &mapping);
        assert!(result.is_some());
        let (agent_id, rest) = result.unwrap();
        assert_eq!(agent_id, "marketing"); // First persona (nike -> marketing)
        assert_eq!(rest, "작업"); // Rest is after all extracted personas (new behavior)
    }
