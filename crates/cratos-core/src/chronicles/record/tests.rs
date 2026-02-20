
    use super::*;

    #[test]
    fn test_chronicle_new() {
        let chronicle = Chronicle::new("sindri");
        assert_eq!(chronicle.persona_name, "sindri");
        assert_eq!(chronicle.level, 1);
        assert_eq!(chronicle.status, ChronicleStatus::Active);
        assert!(chronicle.log.is_empty());
        assert!(chronicle.quests.is_empty());
    }

    #[test]
    fn test_add_entry() {
        let mut chronicle = Chronicle::new("sindri");
        chronicle.add_entry("API implementation complete", Some("2"));

        assert_eq!(chronicle.log.len(), 1);
        assert_eq!(chronicle.log[0].achievement, "API implementation complete");
        assert_eq!(chronicle.log[0].law_reference, Some("2".to_string()));
    }

    #[test]
    fn test_add_entry_with_commit() {
        let mut chronicle = Chronicle::new("sindri");
        chronicle.add_entry_with_commit("Bug fix", Some("10"), "abc123");

        assert_eq!(chronicle.log.len(), 1);
        assert_eq!(chronicle.log[0].commit_hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_quest_management() {
        let mut chronicle = Chronicle::new("sindri");
        chronicle.add_quest("Implement REST API");
        chronicle.add_quest("Design DB schema");

        assert_eq!(chronicle.quests.len(), 2);
        assert_eq!(chronicle.pending_quests(), 2);
        assert_eq!(chronicle.completed_quests(), 0);

        chronicle.complete_quest(0);

        assert_eq!(chronicle.pending_quests(), 1);
        assert_eq!(chronicle.completed_quests(), 1);
        assert!(chronicle.quests[0].completed);
        assert!(chronicle.quests[0].completed_at.is_some());
    }

    #[test]
    fn test_judgment_and_rating() {
        let mut chronicle = Chronicle::new("sindri");

        chronicle.add_judgment("Heimdall", "Excellent code quality", Some(4.5));
        chronicle.add_judgment("user", "Good job", Some(5.0));

        assert_eq!(chronicle.judgments.len(), 2);
        assert!(chronicle.rating.is_some());

        let rating = chronicle.rating.unwrap();
        assert!((rating - 4.75).abs() < 0.01);
    }

    #[test]
    fn test_promotion_eligibility() {
        let mut chronicle = Chronicle::new("sindri");

        // Lv1 → Lv2: needs 10 entries
        assert!(!chronicle.is_promotion_eligible());
        assert_eq!(chronicle.entries_until_promotion(), 10);

        for i in 0..10 {
            chronicle.add_entry(&format!("Task {i}"), None);
        }

        // No judgments yet → rating is None → eligible (fresh persona grace)
        assert!(chronicle.is_promotion_eligible());
        assert_eq!(chronicle.entries_until_promotion(), 0);
    }

    #[test]
    fn test_promotion_blocked_by_low_rating() {
        let mut chronicle = Chronicle::new("sindri");

        for i in 0..10 {
            chronicle.add_entry(&format!("Task {i}"), None);
        }

        // Add low-score judgments → average below 3.5
        chronicle.add_judgment("Cratos", "Format violation", Some(1.0));
        chronicle.add_judgment("Cratos", "Missing commit hash", Some(2.0));
        // average = 1.5 → below 3.5

        assert!(!chronicle.is_promotion_eligible());
        assert!((chronicle.rating_gap() - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_promotion_allowed_with_good_rating() {
        let mut chronicle = Chronicle::new("sindri");

        for i in 0..10 {
            chronicle.add_entry(&format!("Task {i}"), None);
        }

        chronicle.add_judgment("Heimdall", "Excellent work", Some(4.5));
        chronicle.add_judgment("user", "Good job", Some(5.0));
        // average = 4.75 → above 3.5

        assert!(chronicle.is_promotion_eligible());
        assert!((chronicle.rating_gap()).abs() < 0.01);
    }

    #[test]
    fn test_promote() {
        let mut chronicle = Chronicle::new("sindri");

        // Not eligible for promotion
        assert!(!chronicle.promote());
        assert_eq!(chronicle.level, 1);

        // Add 10 entries (Lv1 → Lv2 condition)
        for i in 0..10 {
            chronicle.add_entry(&format!("Task {i}"), None);
        }

        assert!(chronicle.promote());
        assert_eq!(chronicle.level, 2);
        // Promotion entry also added
        assert!(chronicle
            .log
            .last()
            .unwrap()
            .achievement
            .contains("Promoted to Lv2"));
    }

    #[test]
    fn test_promote_blocked_by_rating() {
        let mut chronicle = Chronicle::new("sindri");

        for i in 0..10 {
            chronicle.add_entry(&format!("Task {i}"), None);
        }

        // Bad rating
        chronicle.add_judgment("Cratos", "Violation", Some(1.0));
        assert!(!chronicle.promote());
        assert_eq!(chronicle.level, 1);

        // Improve rating past threshold
        chronicle.add_judgment("Heimdall", "Great recovery", Some(5.0));
        chronicle.add_judgment("user", "Solid work", Some(5.0));
        // average = (1+5+5)/3 ≈ 3.67 → above 3.5
        assert!(chronicle.promote());
        assert_eq!(chronicle.level, 2);
    }

    #[test]
    fn test_silence_punishment() {
        let mut chronicle = Chronicle::new("sindri");

        chronicle.apply_silence();
        assert_eq!(chronicle.status, ChronicleStatus::Silenced);

        chronicle.release_silence();
        assert_eq!(chronicle.status, ChronicleStatus::Active);
    }

    #[test]
    fn test_quest_new() {
        let quest = Quest::new("Test quest");
        assert_eq!(quest.description, "Test quest");
        assert!(!quest.completed);
        assert!(quest.completed_at.is_none());
    }

    #[test]
    fn test_quest_complete() {
        let mut quest = Quest::new("Test");
        quest.complete();

        assert!(quest.completed);
        assert!(quest.completed_at.is_some());
    }

    #[test]
    fn test_chronicle_entry_builder() {
        let entry = ChronicleEntry::new("Task completed")
            .with_law("2")
            .with_commit("abc123");

        assert_eq!(entry.achievement, "Task completed");
        assert_eq!(entry.law_reference, Some("2".to_string()));
        assert_eq!(entry.commit_hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_judgment_score_clamp() {
        let judgment = Judgment::new("test", "comment").with_score(10.0);
        assert_eq!(judgment.score, Some(5.0)); // Clamped to max

        let judgment = Judgment::new("test", "comment").with_score(0.0);
        assert_eq!(judgment.score, Some(1.0)); // Clamped to min
    }

    #[test]
    fn test_chronicle_serialize_deserialize() {
        let mut chronicle = Chronicle::new("sindri");
        chronicle.add_entry("Test", Some("1"));
        chronicle.add_quest("Quest");

        let json = serde_json::to_string(&chronicle).unwrap();
        let deserialized: Chronicle = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.persona_name, chronicle.persona_name);
        assert_eq!(deserialized.log.len(), chronicle.log.len());
        assert_eq!(deserialized.quests.len(), chronicle.quests.len());
    }
