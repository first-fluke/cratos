use cratos_skills::{
    MatchReason, RouterConfig, Skill, SkillCategory, SkillRegistry, SkillRouter, SkillTrigger,
};

async fn create_test_router() -> SkillRouter {
    let registry = SkillRegistry::new();

    // Add a test skill
    let mut skill = Skill::new("file_reader", "Read files", SkillCategory::Custom)
        .with_trigger(
            SkillTrigger::with_keywords(vec!["read".to_string(), "file".to_string()])
                .add_pattern(r"read\s+(?P<path>\S+)")
                .add_intent("file_operation")
                .with_priority(10),
        );
    skill.activate();
    registry.register(skill).await.unwrap();

    // Add another skill
    let mut skill2 = Skill::new("git_committer", "Git commit", SkillCategory::Custom)
        .with_trigger(
            SkillTrigger::with_keywords(vec!["commit".to_string(), "git".to_string()])
                .with_priority(5),
        );
    skill2.activate();
    registry.register(skill2).await.unwrap();

    SkillRouter::new(registry)
}

#[tokio::test]
async fn test_keyword_routing() {
    let mut router = create_test_router().await;

    let results = router.route("please read the file").await;
    assert!(!results.is_empty());
    assert_eq!(results[0].skill.name, "file_reader");
    assert!(results[0].matched_keywords.contains(&"read".to_string()));
}

#[tokio::test]
async fn test_regex_routing() {
    let mut router = create_test_router().await;

    let results = router.route("read /path/to/file.txt").await;
    assert!(!results.is_empty());
    assert_eq!(results[0].skill.name, "file_reader");
    assert_eq!(
        results[0].captured_groups.get("path"),
        Some(&"/path/to/file.txt".to_string())
    );
}

#[tokio::test]
async fn test_best_match() {
    let mut router = create_test_router().await;

    let best = router.route_best("read the file please").await;
    assert!(best.is_some());
    assert_eq!(best.unwrap().skill.name, "file_reader");
}

#[tokio::test]
async fn test_no_match() {
    let mut router = create_test_router().await;

    let _results = router.route("do something completely different").await;
    // May have some low-scoring matches, but best should be None
    let best = router.route_best("do something completely different").await;
    assert!(best.is_none());
}

#[tokio::test]
async fn test_priority_affects_order() {
    let mut router = create_test_router().await;

    // Both skills match "read file commit" but file_reader has higher priority
    let results = router.route("read file and commit").await;
    assert!(results.len() >= 2);
    // Higher priority skill should come first if scores are similar
    let file_reader_pos = results.iter().position(|r| r.skill.name == "file_reader");
    let committer_pos = results.iter().position(|r| r.skill.name == "git_committer");

    // With more keyword matches for file_reader, it should rank higher
    if let (Some(fp), Some(cp)) = (file_reader_pos, committer_pos) {
        // file_reader matches "read" and "file", while git_committer only matches "commit"
        // file_reader should be ranked higher
        assert!(results[fp].score >= results[cp].score || fp < cp);
    }
}

#[tokio::test]
async fn test_case_insensitive() {
    let mut router = create_test_router().await;

    let results_lower = router.route("read file").await;
    let results_upper = router.route("READ FILE").await;

    assert!(!results_lower.is_empty());
    assert!(!results_upper.is_empty());
    assert_eq!(results_lower[0].skill.name, results_upper[0].skill.name);
}

#[test]
fn test_match_reason_serialization() {
    let reason = MatchReason::Keyword("test".to_string());
    let json = serde_json::to_string(&reason).unwrap();
    assert!(json.contains("keyword"));
}

#[tokio::test]
async fn test_security_input_too_long() {
    let registry = SkillRegistry::new();
    let config = RouterConfig {
        max_input_length: 100,
        ..Default::default()
    };
    let mut router = SkillRouter::with_config(registry, config);

    // Input exceeds max length
    let long_input = "a".repeat(200);
    let results = router.route(&long_input).await;

    // Should return empty results (rejected for security)
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_security_regex_pattern_too_long() {
    let registry = SkillRegistry::new();

    // Create skill with overly long regex pattern
    let long_pattern = format!("{}+", "a".repeat(600));
    let mut skill = Skill::new("test", "Test", SkillCategory::Custom)
        .with_trigger(SkillTrigger::with_keywords(vec![]).add_pattern(&long_pattern));
    skill.activate();
    registry.register(skill).await.unwrap();

    let config = RouterConfig {
        max_pattern_length: 500,
        ..Default::default()
    };
    let mut router = SkillRouter::with_config(registry, config);

    // Pattern should be skipped due to length
    let results = router.route("aaaaaa").await;
    assert!(results.is_empty());
}
