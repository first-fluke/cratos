use super::SkillStore;
use crate::analyzer::{DetectedPattern, PatternStatus};
use crate::skill::{Skill, SkillCategory, SkillOrigin, SkillStatus, SkillStep, SkillTrigger};
use chrono::Utc;
use uuid::Uuid;

async fn create_test_store() -> SkillStore {
    SkillStore::in_memory().await.unwrap()
}

fn create_test_skill() -> Skill {
    Skill::new("test_skill", "A test skill", SkillCategory::Custom)
        .with_trigger(SkillTrigger::with_keywords(vec!["test".to_string()]))
        .with_step(SkillStep::new(
            1,
            "file_read",
            serde_json::json!({"path": "{{path}}"}),
        ))
}

fn create_test_pattern() -> DetectedPattern {
    DetectedPattern {
        id: Uuid::new_v4(),
        tool_sequence: vec!["file_read".to_string(), "git_commit".to_string()],
        occurrence_count: 5,
        confidence_score: 0.8,
        extracted_keywords: vec!["read".to_string()],
        sample_inputs: vec!["test input".to_string()],
        status: PatternStatus::Detected,
        converted_skill_id: None,
        detected_at: Utc::now(),
    }
}

#[tokio::test]
async fn test_save_and_get_skill() {
    let store = create_test_store().await;
    let skill = create_test_skill();

    store.save_skill(&skill).await.unwrap();

    let retrieved = store.get_skill(skill.id).await.unwrap();
    assert_eq!(retrieved.name, skill.name);
    assert_eq!(retrieved.steps.len(), 1);
}

#[tokio::test]
async fn test_get_skill_by_name() {
    let store = create_test_store().await;
    let skill = create_test_skill();

    store.save_skill(&skill).await.unwrap();

    let retrieved = store.get_skill_by_name("test_skill").await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, skill.id);
}

#[tokio::test]
async fn test_list_active_skills() {
    let store = create_test_store().await;

    let mut skill1 = create_test_skill();
    skill1.name = "skill1".to_string();
    skill1.activate();
    store.save_skill(&skill1).await.unwrap();

    let mut skill2 = create_test_skill();
    skill2.name = "skill2".to_string();
    skill2.id = Uuid::new_v4();
    // skill2 is draft (default)
    store.save_skill(&skill2).await.unwrap();

    let active = store.list_active_skills().await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "skill1");
}

#[tokio::test]
async fn test_delete_skill() {
    let store = create_test_store().await;
    let skill = create_test_skill();

    store.save_skill(&skill).await.unwrap();
    assert!(store.get_skill(skill.id).await.is_ok());

    store.delete_skill(skill.id).await.unwrap();
    assert!(store.get_skill(skill.id).await.is_err());
}

#[tokio::test]
async fn test_save_and_get_pattern() {
    let store = create_test_store().await;
    let pattern = create_test_pattern();

    store.save_pattern(&pattern).await.unwrap();

    let retrieved = store.get_pattern(pattern.id).await.unwrap();
    assert_eq!(retrieved.tool_sequence, pattern.tool_sequence);
    assert_eq!(retrieved.occurrence_count, pattern.occurrence_count);
}

#[tokio::test]
async fn test_list_detected_patterns() {
    let store = create_test_store().await;

    let pattern1 = create_test_pattern();
    store.save_pattern(&pattern1).await.unwrap();

    let mut pattern2 = create_test_pattern();
    pattern2.id = Uuid::new_v4();
    pattern2.status = PatternStatus::Converted;
    store.save_pattern(&pattern2).await.unwrap();

    let detected = store.list_detected_patterns().await.unwrap();
    assert_eq!(detected.len(), 1);
    assert_eq!(detected[0].id, pattern1.id);
}

#[tokio::test]
async fn test_mark_pattern_converted() {
    let store = create_test_store().await;
    let pattern = create_test_pattern();

    // Create a skill first to satisfy foreign key constraint
    let skill = create_test_skill();
    store.save_skill(&skill).await.unwrap();

    store.save_pattern(&pattern).await.unwrap();
    store
        .mark_pattern_converted(pattern.id, skill.id)
        .await
        .unwrap();

    let updated = store.get_pattern(pattern.id).await.unwrap();
    assert_eq!(updated.status, PatternStatus::Converted);
    assert_eq!(updated.converted_skill_id, Some(skill.id));
}

#[tokio::test]
async fn test_record_skill_execution() {
    let store = create_test_store().await;
    let skill = create_test_skill();
    store.save_skill(&skill).await.unwrap();

    store
        .record_skill_execution(skill.id, None, true, Some(100), &[])
        .await
        .unwrap();

    store
        .record_skill_execution(skill.id, None, false, Some(50), &[])
        .await
        .unwrap();

    let (total, successes) = store.get_skill_execution_count(skill.id).await.unwrap();
    assert_eq!(total, 2);
    assert_eq!(successes, 1);
}

#[tokio::test]
async fn test_update_skill_metrics() {
    let store = create_test_store().await;
    let mut skill = create_test_skill();
    skill.activate();
    store.save_skill(&skill).await.unwrap();

    store
        .update_skill_metrics(skill.id, 10, 0.8, Some(150), "active")
        .await
        .unwrap();

    let updated = store.get_skill(skill.id).await.unwrap();
    assert_eq!(updated.metadata.usage_count, 10);
    assert!((updated.metadata.success_rate - 0.8).abs() < 0.01);
    assert_eq!(updated.metadata.avg_duration_ms, Some(150));
    assert_eq!(updated.status, SkillStatus::Active);
}

#[tokio::test]
async fn test_update_skill_metrics_auto_disable() {
    let store = create_test_store().await;
    let mut skill = create_test_skill();
    skill.activate();
    store.save_skill(&skill).await.unwrap();

    // Simulate low success rate → disabled status
    store
        .update_skill_metrics(skill.id, 15, 0.2, Some(100), "disabled")
        .await
        .unwrap();

    let updated = store.get_skill(skill.id).await.unwrap();
    assert_eq!(updated.status, SkillStatus::Disabled);
    assert!((updated.metadata.success_rate - 0.2).abs() < 0.01);
}

#[tokio::test]
async fn test_list_stale_skills() {
    let store = create_test_store().await;

    // Skill 1: Recently used (1 day ago)
    let mut skill1 = create_test_skill();
    skill1.name = "recent".to_string();
    skill1.metadata.last_used_at = Some(Utc::now() - chrono::Duration::days(1));
    store.save_skill(&skill1).await.unwrap();

    // Skill 2: Stale (100 days ago)
    let mut skill2 = create_test_skill();
    skill2.id = Uuid::new_v4();
    skill2.name = "stale".to_string();
    skill2.metadata.last_used_at = Some(Utc::now() - chrono::Duration::days(100));
    store.save_skill(&skill2).await.unwrap();

    // Skill 3: Builtin (should not be stale even if old)
    let mut skill3 = create_test_skill();
    skill3.id = Uuid::new_v4();
    skill3.name = "builtin".to_string();
    skill3.origin = SkillOrigin::Builtin;
    skill3.metadata.last_used_at = Some(Utc::now() - chrono::Duration::days(100));
    store.save_skill(&skill3).await.unwrap();

    let stale = store.list_stale_skills(90).await.unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].name, "stale");
}

#[tokio::test]
async fn test_prune_stale_skills() {
    let store = create_test_store().await;

    let mut skill1 = create_test_skill();
    skill1.metadata.last_used_at = Some(Utc::now() - chrono::Duration::days(100));
    store.save_skill(&skill1).await.unwrap();

    let count = store.prune_stale_skills(90).await.unwrap();
    assert_eq!(count, 1);

    let stale = store.list_stale_skills(90).await.unwrap();
    assert_eq!(stale.len(), 0);
}
