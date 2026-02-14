//! Event conversion for the Gateway WebSocket.

use cratos_core::event_bus::OrchestratorEvent;

use crate::websocket::protocol::GatewayFrame;

/// Convert an OrchestratorEvent to a GatewayFrame::Event.
/// Returns None for events we don't forward.
/// Public for ACP bridge reuse.
pub fn convert_event(event: &OrchestratorEvent) -> Option<GatewayFrame> {
    let (name, data) = match event {
        OrchestratorEvent::ExecutionStarted {
            execution_id,
            session_key,
        } => (
            "execution.started",
            serde_json::json!({
                "execution_id": execution_id,
                "session_key": session_key,
            }),
        ),
        OrchestratorEvent::PlanningStarted {
            execution_id,
            iteration,
        } => (
            "execution.planning",
            serde_json::json!({
                "execution_id": execution_id,
                "iteration": iteration,
            }),
        ),
        OrchestratorEvent::ChatDelta {
            execution_id,
            delta,
            is_final,
        } => (
            "chat.delta",
            serde_json::json!({
                "execution_id": execution_id,
                "delta": delta,
                "is_final": is_final,
            }),
        ),
        OrchestratorEvent::ToolStarted {
            execution_id,
            tool_name,
            tool_call_id,
        } => (
            "tool.started",
            serde_json::json!({
                "execution_id": execution_id,
                "tool_name": tool_name,
                "tool_call_id": tool_call_id,
            }),
        ),
        OrchestratorEvent::ToolCompleted {
            execution_id,
            tool_call_id,
            tool_name,
            success,
            duration_ms,
        } => (
            "tool.completed",
            serde_json::json!({
                "execution_id": execution_id,
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "success": success,
                "duration_ms": duration_ms,
            }),
        ),
        OrchestratorEvent::ApprovalRequired {
            execution_id,
            request_id,
        } => (
            "approval.required",
            serde_json::json!({
                "execution_id": execution_id,
                "request_id": request_id,
            }),
        ),
        OrchestratorEvent::ExecutionCompleted { execution_id } => (
            "execution.completed",
            serde_json::json!({"execution_id": execution_id}),
        ),
        OrchestratorEvent::ExecutionFailed {
            execution_id,
            error,
        } => (
            "execution.failed",
            serde_json::json!({
                "execution_id": execution_id,
                "error": error,
            }),
        ),
        OrchestratorEvent::ExecutionCancelled { execution_id } => (
            "execution.cancelled",
            serde_json::json!({"execution_id": execution_id}),
        ),
        OrchestratorEvent::A2aMessageSent {
            session_id,
            from_agent,
            to_agent,
            message_id,
        } => (
            "a2a.message",
            serde_json::json!({
                "session_id": session_id,
                "from_agent": from_agent,
                "to_agent": to_agent,
                "message_id": message_id,
            }),
        ),
        OrchestratorEvent::QuotaWarning {
            provider,
            remaining_pct,
            reset_in_secs,
        } => (
            "quota.warning",
            serde_json::json!({
                "provider": provider,
                "remaining_pct": remaining_pct,
                "reset_in_secs": reset_in_secs,
            }),
        ),
    };

    Some(GatewayFrame::event(name, data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_convert_all_events() {
        let id = Uuid::new_v4();
        let events = vec![
            OrchestratorEvent::ExecutionStarted {
                execution_id: id,
                session_key: "s".to_string(),
            },
            OrchestratorEvent::PlanningStarted {
                execution_id: id,
                iteration: 1,
            },
            OrchestratorEvent::ChatDelta {
                execution_id: id,
                delta: "hi".to_string(),
                is_final: false,
            },
            OrchestratorEvent::ToolStarted {
                execution_id: id,
                tool_name: "exec".to_string(),
                tool_call_id: "c1".to_string(),
            },
            OrchestratorEvent::ToolCompleted {
                execution_id: id,
                tool_call_id: "c1".to_string(),
                tool_name: "exec".to_string(),
                success: true,
                duration_ms: 50,
            },
            OrchestratorEvent::ApprovalRequired {
                execution_id: id,
                request_id: Uuid::new_v4(),
            },
            OrchestratorEvent::ExecutionCompleted { execution_id: id },
            OrchestratorEvent::ExecutionFailed {
                execution_id: id,
                error: "oops".to_string(),
            },
            OrchestratorEvent::ExecutionCancelled { execution_id: id },
            OrchestratorEvent::A2aMessageSent {
                session_id: "s1".to_string(),
                from_agent: "backend".to_string(),
                to_agent: "frontend".to_string(),
                message_id: Uuid::new_v4(),
            },
        ];

        for event in events {
            let frame = convert_event(&event);
            assert!(frame.is_some(), "event should convert: {:?}", event);
            let json = serde_json::to_string(&frame.unwrap()).unwrap();
            assert!(json.contains("\"frame\":\"event\""));
        }
    }
}
