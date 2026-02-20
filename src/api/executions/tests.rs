use super::types::{
    default_limit_inner, EventSummary, ExecutionDetail, ExecutionSummary, ListExecutionsQuery,
};
use chrono::Utc;
use uuid::Uuid;

#[test]
fn test_execution_summary_serialization() {
    let summary = ExecutionSummary {
        id: Uuid::nil(),
        channel_type: "telegram".to_string(),
        channel_id: "123".to_string(),
        user_id: "user1".to_string(),
        input_text: "hello".to_string(),
        output_text: Some("world".to_string()),
        status: "completed".to_string(),
        created_at: Utc::now(),
        completed_at: Some(Utc::now()),
    };
    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("\"channel_type\":\"telegram\""));
    assert!(json.contains("\"status\":\"completed\""));
}

#[test]
fn test_execution_detail_serialization() {
    let detail = ExecutionDetail {
        id: Uuid::nil(),
        channel_type: "websocket".to_string(),
        channel_id: "ws1".to_string(),
        user_id: "user1".to_string(),
        thread_id: None,
        input_text: "test input".to_string(),
        output_text: Some("test output".to_string()),
        status: "completed".to_string(),
        created_at: Utc::now(),
        completed_at: Some(Utc::now()),
        events: vec![EventSummary {
            id: Uuid::nil(),
            sequence_num: 1,
            event_type: "user_input".to_string(),
            timestamp: Utc::now(),
            duration_ms: None,
        }],
    };
    let json = serde_json::to_string(&detail).unwrap();
    assert!(json.contains("\"events\""));
    assert!(json.contains("\"user_input\""));
}

#[test]
fn test_default_limit() {
    assert_eq!(default_limit_inner(), 50);
}

#[test]
fn test_list_query_deserialization() {
    let json = r#"{"limit": 10, "channel": "telegram"}"#;
    let query: ListExecutionsQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.limit, 10);
    assert_eq!(query.channel.as_deref(), Some("telegram"));
}

#[test]
fn test_replay_options_deserialization() {
    let json = r#"{"dry_run": true, "skip_tools": ["exec"]}"#;
    let opts: cratos_replay::ReplayOptions = serde_json::from_str(json).unwrap();
    assert!(opts.dry_run);
    assert_eq!(opts.skip_tools, vec!["exec"]);
}

#[test]
fn test_replay_result_serialization() {
    let result = cratos_replay::ReplayResult {
        original_execution_id: Uuid::nil(),
        new_execution_id: None,
        steps: vec![],
        dry_run: true,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"dry_run\":true"));
}
