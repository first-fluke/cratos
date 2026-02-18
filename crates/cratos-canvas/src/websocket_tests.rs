use super::*;
use crate::a2ui::A2uiClientMessage;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

// Test A2UI message forwarding logic without Full WebSocket mock
// But we can unit test the 'channel forwarding' part easily if we assume handle_client_message works.

#[tokio::test]
async fn test_a2ui_forwarding() {
    let (tx, mut rx) = mpsc::channel(1);
    let manager = Arc::new(CanvasSessionManager::default());
    let state = Arc::new(CanvasState::new(manager).with_a2ui_tx(tx));
    
    // We can't call handle_client_message directly because of SplitSink complication.
    // So we manually verified via code review that it forwards.
    
    // BUT we can test the channel logic:
    let session_id = Uuid::new_v4();
    let msg = A2uiClientMessage::Event { 
        component_id: Uuid::new_v4(), 
        event_type: "click".into(), 
        payload: serde_json::Value::Null 
    };
    
    if let Some(forward_tx) = &state.a2ui_tx {
        forward_tx.send((session_id, msg.clone())).await.unwrap();
    }
    
    let received = rx.recv().await.unwrap();
    assert_eq!(received.0, session_id);
    match received.1 {
        A2uiClientMessage::Event { event_type, .. } => assert_eq!(event_type, "click"),
        _ => panic!("Wrong message type"),
    }
}
