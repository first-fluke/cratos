//! Tests for store module

use super::*;
use crate::event::{EventType, Execution, ExecutionStatus};

#[test]
fn test_execution_query_builder() {
    let query = ExecutionQuery::new()
        .for_channel("telegram", "123")
        .for_user("user1")
        .with_status(ExecutionStatus::Completed)
        .paginate(10, 0);

    assert_eq!(query.channel_type, Some("telegram".to_string()));
    assert_eq!(query.channel_id, Some("123".to_string()));
    assert_eq!(query.user_id, Some("user1".to_string()));
    assert_eq!(query.status, Some(ExecutionStatus::Completed));
    assert_eq!(query.limit, 10);
    assert_eq!(query.offset, 0);
}

#[test]
fn test_default_data_dir() {
    let dir = default_data_dir();
    assert!(dir.to_string_lossy().contains("cratos"));
}

#[tokio::test]
async fn test_in_memory_store() {
    let store = EventStore::in_memory().await.unwrap();
    assert_eq!(store.name(), "sqlite");

    // Create an execution
    let execution = Execution::new("telegram", "12345", "user1", "Hello, world!");
    store.create_execution(&execution).await.unwrap();

    // Retrieve it
    let retrieved = store.get_execution(execution.id).await.unwrap();
    assert_eq!(retrieved.id, execution.id);
    assert_eq!(retrieved.input_text, "Hello, world!");
}

#[tokio::test]
async fn test_event_recording() {
    let store = EventStore::in_memory().await.unwrap();

    // Create an execution
    let execution = Execution::new("telegram", "12345", "user1", "Hello");
    store.create_execution(&execution).await.unwrap();

    // Create a recorder
    let recorder = EventRecorder::new(store.clone(), execution.id);

    // Record some events
    recorder.record_user_input("Hello").await.unwrap();
    recorder
        .record_llm_request("openai", "gpt-4", 1, &[])
        .await
        .unwrap();
    recorder
        .record_llm_response("openai", "gpt-4", "Hi!", false, None, 100)
        .await
        .unwrap();

    // Verify
    let events = get_execution_events(&store, execution.id).await.unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, EventType::UserInput);
    assert_eq!(events[1].event_type, EventType::LlmRequest);
    assert_eq!(events[2].event_type, EventType::LlmResponse);
}
