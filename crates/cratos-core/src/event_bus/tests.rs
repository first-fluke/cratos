use super::*;
use uuid::Uuid;

#[tokio::test]
async fn test_publish_subscribe() {
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    let exec_id = Uuid::new_v4();
    bus.publish(OrchestratorEvent::ExecutionStarted {
        execution_id: exec_id,
        session_key: "test:1:1".to_string(),
    });

    let event = rx.recv().await.unwrap();
    assert_eq!(event.execution_id(), exec_id);
    match event {
        OrchestratorEvent::ExecutionStarted { session_key, .. } => {
            assert_eq!(session_key, "test:1:1");
        }
        _ => panic!("unexpected event type"),
    }
}

#[tokio::test]
async fn test_multiple_subscribers() {
    let bus = EventBus::new(16);
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();

    assert_eq!(bus.subscriber_count(), 2);

    let exec_id = Uuid::new_v4();
    let count = bus.publish(OrchestratorEvent::ExecutionCompleted {
        execution_id: exec_id,
    });
    assert_eq!(count, 2);

    let e1 = rx1.recv().await.unwrap();
    let e2 = rx2.recv().await.unwrap();
    assert_eq!(e1.execution_id(), exec_id);
    assert_eq!(e2.execution_id(), exec_id);
}

#[test]
fn test_event_serialization() {
    let event = OrchestratorEvent::ToolStarted {
        execution_id: Uuid::nil(),
        tool_name: "file_read".to_string(),
        tool_call_id: "call_1".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"tool_started\""));
    assert!(json.contains("\"tool_name\":\"file_read\""));
}
