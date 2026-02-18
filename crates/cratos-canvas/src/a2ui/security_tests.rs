use super::*;
use crate::a2ui::protocol::*;
use serde_json::json;

#[test]
fn test_security_blocks_script_tags() {
    let policy = A2uiSecurityPolicy::default_restrictive();
    let msg = A2uiServerMessage::Render {
        component_id: uuid::Uuid::new_v4(),
        component_type: A2uiComponentType::Markdown,
        props: json!({"content": "<script>alert('xss')</script>"}),
        slot: None,
    };
    
    assert!(matches!(
        policy.validate(&msg),
        Err(A2uiSecurityError::XssAttempt(_))
    ));
}

#[test]
fn test_security_blocks_javascript_url() {
    let policy = A2uiSecurityPolicy::default_restrictive();
    let msg = A2uiServerMessage::Navigate {
        url: "javascript:alert(1)".into(),
        options: NavigateOptions { // imported from protocol
            target: NavigateTarget::NewTab,
            sandbox: true,
        },
    };
    
    assert!(matches!(
        policy.validate(&msg),
        Err(A2uiSecurityError::UnsafeScheme(_))
    ));
}

#[test]
fn test_allow_safe_component() {
    let policy = A2uiSecurityPolicy::default_restrictive();
    let msg = A2uiServerMessage::Render {
        component_id: uuid::Uuid::new_v4(),
        component_type: A2uiComponentType::Text,
        props: json!({"content": "Hello A2UI"}),
        slot: None,
    };
    assert!(policy.validate(&msg).is_ok());
}

#[test]
fn test_block_unsafe_component() {
    let policy = A2uiSecurityPolicy::default_restrictive();
    let msg = A2uiServerMessage::Render {
        component_id: uuid::Uuid::new_v4(),
        component_type: A2uiComponentType::Iframe,
        props: json!({"src": "https://example.com"}),
        slot: None,
    };
    // Expect fail because default policy disallows iframes
    assert!(matches!(
        policy.validate(&msg),
        Err(A2uiSecurityError::ComponentNotAllowed(_))
    ));
}
