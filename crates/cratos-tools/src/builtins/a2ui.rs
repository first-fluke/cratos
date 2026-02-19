use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use cratos_canvas::a2ui::{
    A2uiClientMessage, A2uiComponentType, A2uiSecurityPolicy, A2uiServerMessage, A2uiSessionManager,
};

/// Tool for rendering UI components to the client via A2UI protocol.
pub struct A2uiRenderTool {
    definition: ToolDefinition,
    session_manager: Arc<A2uiSessionManager>,
    security_policy: Arc<A2uiSecurityPolicy>,
}

impl A2uiRenderTool {
    /// Create a new A2UI render tool
    pub fn new(
        session_manager: Arc<A2uiSessionManager>,
        security_policy: Arc<A2uiSecurityPolicy>,
    ) -> Self {
        let definition = ToolDefinition::new(
            "a2ui_render",
            "Render UI components in the user's browser/app. Returns component_id for event handling.",
        )
        .with_category(ToolCategory::Utility)
        .with_risk_level(RiskLevel::Medium)
        .with_parameters(json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "ID of the target session"
                },
                "component_type": {
                    "type": "string",
                    "description": "Type of UI component to render",
                    "enum": [
                        "text_input", "text_area", "select", "checkbox", "radio", 
                        "text", "markdown", "code", "image", "card", "modal", 
                        "table", "chart", "button", "form"
                    ]
                },
                "props": {
                    "type": "object",
                    "description": "Component properties (content, style, options, etc.)"
                },
                "slot": {
                    "type": "string",
                    "description": "Target slot: 'main', 'sidebar', 'modal'"
                }
            },
            "required": ["session_id", "component_type", "props"]
        }));

        Self {
            definition,
            session_manager,
            security_policy,
        }
    }
}

#[async_trait]
impl Tool for A2uiRenderTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let start_time = std::time::Instant::now();

        let session_id_str = args["session_id"]
            .as_str()
            .ok_or_else(|| Error::InvalidInput("Missing session_id".into()))?;
        let session_id = Uuid::parse_str(session_id_str)
            .map_err(|e| Error::InvalidInput(format!("Invalid session_id UUID: {}", e)))?;

        let component_type_str = args["component_type"]
            .as_str()
            .ok_or_else(|| Error::InvalidInput("Missing component_type".into()))?;

        let component_type: A2uiComponentType =
            serde_json::from_value(json!(component_type_str))
                .map_err(|e| Error::InvalidInput(format!("Invalid component_type: {}", e)))?;

        let props = args["props"].clone();
        let slot = args.get("slot").and_then(|s| s.as_str()).map(String::from);

        let msg = A2uiServerMessage::Render {
            component_id: Uuid::new_v4(),
            component_type,
            props,
            slot,
        };

        if let Err(e) = self.security_policy.validate(&msg) {
            return Ok(ToolResult::failure(
                format!("Security Policy Violation: {}", e),
                start_time.elapsed().as_millis() as u64,
            ));
        }

        let session = match self.session_manager.get_or_create(session_id).await {
            Ok(s) => s,
            Err(e) => {
                return Ok(ToolResult::failure(
                    format!("Session Error: {}", e),
                    start_time.elapsed().as_millis() as u64,
                ))
            }
        };

        if let Err(e) = session.send(msg.clone()).await {
            return Ok(ToolResult::failure(
                format!("Send Error: {}", e),
                start_time.elapsed().as_millis() as u64,
            ));
        }

        let component_id = match msg {
            A2uiServerMessage::Render { component_id, .. } => component_id,
            _ => unreachable!(),
        };

        Ok(ToolResult::success(
            json!({
                "component_id": component_id,
                "rendered": true
            }),
            start_time.elapsed().as_millis() as u64,
        ))
    }
}

/// Tool for waiting for user interaction events on a rendered A2UI component.
pub struct A2uiWaitEventTool {
    definition: ToolDefinition,
    session_manager: Arc<A2uiSessionManager>,
}

impl A2uiWaitEventTool {
    /// Create a new A2UI wait event tool
    pub fn new(session_manager: Arc<A2uiSessionManager>) -> Self {
        let definition = ToolDefinition::new(
            "a2ui_wait_event",
            "Wait for user interaction with a rendered component.",
        )
        .with_category(ToolCategory::Utility)
        .with_parameters(json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "ID of the target session"
                },
                "component_id": {
                    "type": "string",
                    "description": "ID of the component to wait for events from"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default: 30000)"
                }
            },
            "required": ["session_id", "component_id"]
        }));

        Self {
            definition,
            session_manager,
        }
    }
}

#[async_trait]
impl Tool for A2uiWaitEventTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let start_time = std::time::Instant::now();

        let session_id_str = args["session_id"]
            .as_str()
            .ok_or_else(|| Error::InvalidInput("Missing session_id".into()))?;
        let session_id = Uuid::parse_str(session_id_str)
            .map_err(|e| Error::InvalidInput(format!("Invalid session_id UUID: {}", e)))?;

        let component_id_str = args["component_id"]
            .as_str()
            .ok_or_else(|| Error::InvalidInput("Missing component_id".into()))?;
        let component_id = Uuid::parse_str(component_id_str)
            .map_err(|e| Error::InvalidInput(format!("Invalid component_id UUID: {}", e)))?;

        let timeout_ms = args
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(30000);

        let session = match self.session_manager.get_or_create(session_id).await {
            Ok(s) => s,
            Err(e) => {
                return Ok(ToolResult::failure(
                    format!("Session Error: {}", e),
                    start_time.elapsed().as_millis() as u64,
                ))
            }
        };

        // Wait for event with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            session.wait_event(component_id),
        )
        .await;

        match result {
            Ok(Ok(event)) => match event {
                A2uiClientMessage::Event {
                    event_type,
                    payload,
                    ..
                } => Ok(ToolResult::success(
                    json!({
                        "event_type": event_type,
                        "payload": payload
                    }),
                    start_time.elapsed().as_millis() as u64,
                )),
                _ => Ok(ToolResult::failure(
                    "Received non-event message".to_string(),
                    start_time.elapsed().as_millis() as u64,
                )),
            },
            Ok(Err(e)) => Ok(ToolResult::failure(
                format!("Wait Error: {}", e),
                start_time.elapsed().as_millis() as u64,
            )),
            Err(_) => Ok(ToolResult::failure(
                "Timeout waiting for event".to_string(),
                start_time.elapsed().as_millis() as u64,
            )),
        }
    }
}
