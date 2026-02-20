use super::protocol::*;
use uuid::Uuid;

#[test]
fn test_client_message_deserialization() {
    let json = r#"{"type":"chat","text":"Hello","persona":null}"#;
    let msg: ClientMessage = serde_json::from_str(json).unwrap();
    assert!(matches!(msg, ClientMessage::Chat { text, .. } if text == "Hello"));
}

#[test]
fn test_server_message_serialization() {
    let msg = ServerMessage::ChatResponse {
        execution_id: Uuid::nil(),
        text: "Hi".to_string(),
        is_final: true,
        persona: "cratos".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"type\":\"chat_response\""));
    assert!(json.contains("\"is_final\":true"));
}

#[test]
fn test_status_message_serialization() {
    let msg = ServerMessage::Status {
        connected: true,
        active_executions: 2,
        persona: "cratos".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"active_executions\":2"));
}

#[test]
fn test_ping_deserialization() {
    let json = r#"{"type":"ping"}"#;
    let msg: ClientMessage = serde_json::from_str(json).unwrap();
    assert!(matches!(msg, ClientMessage::Ping));
}

#[test]
fn test_cancel_deserialization() {
    let id = Uuid::new_v4();
    let json = format!(r#"{{"type":"cancel","execution_id":"{}"}}"#, id);
    let msg: ClientMessage = serde_json::from_str(&json).unwrap();
    assert!(matches!(msg, ClientMessage::Cancel { execution_id } if execution_id == Some(id)));
}

#[test]
fn test_artifact_message_serialization() {
    let msg = ServerMessage::Artifact {
        execution_id: Uuid::nil(),
        filename: "test.png".to_string(),
        mime_type: "image/png".to_string(),
        data: "base64data".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"type\":\"artifact\""));
    assert!(json.contains("\"filename\":\"test.png\""));
    assert!(json.contains("\"mime_type\":\"image/png\""));
}
