//! A2UI Steering Loop
//!
//! Handles steering commands from the A2UI protocol.

use cratos_canvas::a2ui::A2uiClientMessage;
use cratos_core::Orchestrator;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, warn};
use uuid::Uuid;

/// A2UI message type
pub type A2uiMessage = (Uuid, A2uiClientMessage);

/// Start the A2UI steering loop
pub fn start_a2ui_steering_loop(
    orchestrator: Arc<Orchestrator>,
    mut a2ui_rx: mpsc::Receiver<A2uiMessage>,
) {
    tokio::spawn(async move {
        debug!("A2UI Steering Loop started");
        while let Some((_session_id, msg)) = a2ui_rx.recv().await {
            if let A2uiClientMessage::Steer { action, payload } = msg {
                // Determine Execution ID from payload
                let execution_id = payload
                    .as_ref()
                    .and_then(|p| p.get("execution_id"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok());

                if let Some(exec_id) = execution_id {
                    if let Some(handle) = orchestrator.get_steer_handle(exec_id) {
                        debug!(execution_id = %exec_id, action = %action, "Processing Steer command");
                        let result = match action.as_str() {
                            "abort" => {
                                let reason = payload
                                    .as_ref()
                                    .and_then(|p| p.get("reason"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                handle.abort(reason).await
                            }
                            "skip" => {
                                if let Some(tid) = payload
                                    .as_ref()
                                    .and_then(|p| p.get("tool_call_id"))
                                    .and_then(|v| v.as_str())
                                {
                                    handle.skip_tool(tid.to_string()).await
                                } else {
                                    warn!("Skip action missing tool_call_id");
                                    Ok(())
                                }
                            }
                            "user_text" => {
                                if let Some(content) = payload
                                    .as_ref()
                                    .and_then(|p| p.get("content"))
                                    .and_then(|v| v.as_str())
                                {
                                    handle.inject_message(content.to_string()).await
                                } else {
                                    warn!("UserText action missing content");
                                    Ok(())
                                }
                            }
                            _ => {
                                warn!("Unknown steer action: {}", action);
                                Ok(())
                            }
                        };

                        if let Err(e) = result {
                            warn!(execution_id = %exec_id, error = %e, "Failed to send steering command");
                        }
                    } else {
                        debug!(execution_id = %exec_id, "Steer handle not found (execution finished?)");
                    }
                } else {
                    debug!("Steer message missing execution_id in payload, ignoring");
                }
            }
        }
        debug!("A2UI Steering Loop finished");
    });
}
