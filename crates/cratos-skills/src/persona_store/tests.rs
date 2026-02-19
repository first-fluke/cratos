use super::*;
async fn create_test_store() -> PersonaSkillStore {
    PersonaSkillStore::in_memory().await.unwrap()
}

#[tokio::test]
async fn test_claim_skill() {
    let store = create_test_store().await;
    let skill_id = Uuid::new_v4();

    let binding = store
        .claim_skill("sindri", skill_id, "api_builder")
        .await
        .unwrap();

    assert_eq!(binding.persona_name, "sindri");
    assert_eq!(binding.skill_id, skill_id);
    assert_eq!(binding.skill_name, "api_builder");
    assert_eq!(binding.ownership_type, OwnershipType::Claimed);

    // Claiming again should return the same binding
    let binding2 = store
        .claim_skill("sindri", skill_id, "api_builder")
        .await
        .unwrap();
    assert_eq!(binding.id, binding2.id);
}

#[tokio::test]
async fn test_create_default_binding() {
    let store = create_test_store().await;
    let skill_id = Uuid::new_v4();

    let binding = store
        .create_default_binding("sindri", skill_id, "rust_dev")
        .await
        .unwrap();

    assert_eq!(binding.ownership_type, OwnershipType::Default);
}

#[tokio::test]
async fn test_release_skill() {
    let store = create_test_store().await;
    let skill_id = Uuid::new_v4();

    store.claim_skill("sindri", skill_id, "test").await.unwrap();

    let released = store.release_skill("sindri", skill_id).await.unwrap();
    assert!(released);

    // Should be gone now
    let binding = store.get_binding("sindri", skill_id).await.unwrap();
    assert!(binding.is_none());

    // Releasing again should return false
    let released2 = store.release_skill("sindri", skill_id).await.unwrap();
    assert!(!released2);
}

#[tokio::test]
async fn test_record_execution() {
    let store = create_test_store().await;
    let skill_id = Uuid::new_v4();

    store.claim_skill("sindri", skill_id, "test").await.unwrap();

    // Record some executions
    store
        .record_execution("sindri", skill_id, true, Some(100))
        .await
        .unwrap();
    store
        .record_execution("sindri", skill_id, true, Some(200))
        .await
        .unwrap();
    store
        .record_execution("sindri", skill_id, false, None)
        .await
        .unwrap();

    let binding = store
        .get_binding("sindri", skill_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.usage_count, 3);
    assert_eq!(binding.success_count, 2);
    assert_eq!(binding.failure_count, 1);
    assert_eq!(binding.consecutive_successes, 0); // Reset by failure
    assert!((binding.success_rate - 0.666).abs() < 0.01);
}

#[tokio::test]
async fn test_auto_assignment() {
    let store = create_test_store().await;
    let skill_id = Uuid::new_v4();
    let config = AutoAssignmentConfig::default();

    store.claim_skill("sindri", skill_id, "test").await.unwrap();

    // Record 5 consecutive successes
    for _ in 0..5 {
        store
            .record_execution("sindri", skill_id, true, Some(100))
            .await
            .unwrap();
    }

    // Check auto-assignment
    let assigned = store
        .check_auto_assignment("sindri", skill_id, &config)
        .await
        .unwrap();
    assert!(assigned);

    // Verify ownership changed
    let binding = store
        .get_binding("sindri", skill_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.ownership_type, OwnershipType::AutoAssigned);
    assert!(binding.auto_assigned_at.is_some());
}

#[tokio::test]
async fn test_get_top_skills() {
    let store = create_test_store().await;

    // Create multiple skills with different success rates
    for i in 0..5 {
        let skill_id = Uuid::new_v4();
        store
            .claim_skill("sindri", skill_id, &format!("skill_{i}"))
            .await
            .unwrap();

        // Record executions
        for _ in 0..3 {
            store
                .record_execution("sindri", skill_id, i != 2, Some(100))
                .await
                .unwrap();
        }
    }

    let top = store.get_top_skills("sindri", 3).await.unwrap();
    assert_eq!(top.len(), 3);

    // All top skills should have 100% success rate (skill_2 had failures)
    for binding in &top {
        assert_eq!(binding.success_rate, 1.0);
    }
}

#[tokio::test]
async fn test_skill_leaderboard() {
    let store = create_test_store().await;
    let skill_id = Uuid::new_v4();

    // Different personas use the same skill
    for persona in ["sindri", "brok", "athena"] {
        store
            .claim_skill(persona, skill_id, "shared_skill")
            .await
            .unwrap();

        for _ in 0..3 {
            let success = persona != "brok"; // brok fails
            store
                .record_execution(persona, skill_id, success, Some(100))
                .await
                .unwrap();
        }
    }

    let leaderboard = store.get_skill_leaderboard(skill_id, 5).await.unwrap();
    assert_eq!(leaderboard.len(), 3);

    // sindri and athena should be ahead of brok
    assert_ne!(leaderboard[0].persona_name, "brok");
    assert_eq!(leaderboard[2].persona_name, "brok");
}

#[tokio::test]
async fn test_has_skill_by_name() {
    let store = create_test_store().await;
    let skill_id = Uuid::new_v4();

    assert!(!store.has_skill_by_name("sindri", "test").await.unwrap());

    store.claim_skill("sindri", skill_id, "test").await.unwrap();

    assert!(store.has_skill_by_name("sindri", "test").await.unwrap());
    assert!(!store.has_skill_by_name("brok", "test").await.unwrap());
}

#[tokio::test]
async fn test_get_auto_assigned_skills() {
    let store = create_test_store().await;
    let skill_id = Uuid::new_v4();
    let config = AutoAssignmentConfig::default();

    store.claim_skill("sindri", skill_id, "test").await.unwrap();

    // Initially no auto-assigned skills
    let auto = store.get_auto_assigned_skills("sindri").await.unwrap();
    assert!(auto.is_empty());

    // Trigger auto-assignment
    for _ in 0..5 {
        store
            .record_execution("sindri", skill_id, true, Some(100))
            .await
            .unwrap();
    }
    store
        .check_auto_assignment("sindri", skill_id, &config)
        .await
        .unwrap();

    let auto = store.get_auto_assigned_skills("sindri").await.unwrap();
    assert_eq!(auto.len(), 1);
    assert_eq!(auto[0].skill_name, "test");
}

#[tokio::test]
async fn test_get_skill_proficiency_map() {
    let store = create_test_store().await;

    // Create skills with different success rates
    for i in 0..3 {
        let skill_id = Uuid::new_v4();
        store
            .claim_skill("sindri", skill_id, &format!("skill_{i}"))
            .await
            .unwrap();

        for j in 0..3 {
            let success = j < (3 - i); // varying success rates
            store
                .record_execution("sindri", skill_id, success, Some(100))
                .await
                .unwrap();
        }
    }

    let map = store.get_skill_proficiency_map("sindri").await.unwrap();
    assert_eq!(map.len(), 3);
    assert!(map.contains_key("skill_0"));
    assert!(map.contains_key("skill_1"));
    assert!(map.contains_key("skill_2"));
}

#[tokio::test]
async fn test_execution_history() {
    let store = create_test_store().await;
    let skill_id = Uuid::new_v4();

    store.claim_skill("sindri", skill_id, "test").await.unwrap();

    for _ in 0..5 {
        store
            .record_execution("sindri", skill_id, true, Some(100))
            .await
            .unwrap();
    }

    let history = store.get_execution_history("sindri", 3).await.unwrap();
    assert_eq!(history.len(), 3);

    // Should be ordered by started_at DESC
    assert!(history[0].started_at >= history[1].started_at);
}
