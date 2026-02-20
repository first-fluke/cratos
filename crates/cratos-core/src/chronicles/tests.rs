
    use super::*;

    #[test]
    fn test_chronicle_new() {
        let chronicle = Chronicle::new("test_persona");
        assert_eq!(chronicle.persona_name, "test_persona");
        assert_eq!(chronicle.level, 1);
        assert_eq!(chronicle.status, ChronicleStatus::Active);
    }

    #[test]
    fn test_chronicle_add_entry() {
        let mut chronicle = Chronicle::new("test");
        chronicle.add_entry("Test task completed", Some("2"));

        assert_eq!(chronicle.log.len(), 1);
        assert_eq!(chronicle.log[0].achievement, "Test task completed");
        assert_eq!(chronicle.log[0].law_reference, Some("2".to_string()));
    }
