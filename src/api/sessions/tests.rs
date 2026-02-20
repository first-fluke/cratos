use super::*;
use axum::Json;
use cratos_core::auth::{AuthContext, AuthMethod, Scope};

fn test_auth() -> RequireAuth {
    RequireAuth(AuthContext {
        user_id: "test".to_string(),
        method: AuthMethod::ApiKey,
        scopes: vec![Scope::Admin],
        session_id: None,
        device_id: None,
    })
}

#[tokio::test]
async fn test_create_and_list_sessions() {
    let state = SessionState::new();

    let response = create_session(
        test_auth(),
        State(state.clone()),
        Json(CreateSessionRequest {
            name: Some("Test Session".to_string()),
        }),
    )
    .await;
    assert!(response.0.success);

    let response = list_sessions(test_auth(), State(state)).await;
    assert!(response.0.success);
    let sessions = response.0.data.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].name, Some("Test Session".to_string()));
}

#[tokio::test]
async fn test_send_message_starts_execution() {
    let state = SessionState::new();

    let session = create_session(
        test_auth(),
        State(state.clone()),
        Json(CreateSessionRequest { name: None }),
    )
    .await;
    let session_id = session.0.data.unwrap().id;

    let response = send_message(
        test_auth(),
        State(state),
        Path(session_id),
        Json(SendMessageRequest {
            text: "Hello".to_string(),
        }),
    )
    .await
    .unwrap();

    assert!(response.0.success);
    let data = response.0.data.unwrap();
    assert!(data.started);
    assert_eq!(data.queue_position, 0);
}
