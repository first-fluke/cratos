//! Orchestrator helper methods
//!
//! Contains utility methods for the Orchestrator:
//! - Event emission and logging
//! - Session management

use crate::event_bus::OrchestratorEvent;
use cratos_replay::{Event, EventType};
use tracing::warn;
use uuid::Uuid;

use super::core::Orchestrator;

impl Orchestrator {
    /// Publish an event to the event bus (no-op if no bus is set).
    pub(crate) fn emit(&self, event: OrchestratorEvent) {
        if let Some(bus) = &self.event_bus {
            bus.publish(event);
        }
    }

    /// Log an event to the event store
    pub(crate) async fn log_event(
        &self,
        execution_id: Uuid,
        event_type: EventType,
        payload: &serde_json::Value,
    ) {
        if !self.config.enable_logging {
            return;
        }

        if let Some(store) = &self.event_store {
            // Use the Event builder pattern from cratos-replay
            let event = Event::new(execution_id, 0, event_type).with_payload(payload.clone());

            if let Err(e) = store.append(event).await {
                warn!(error = %e, "Failed to log event");
            }
        }
    }

    /// Clear a user's session
    pub async fn clear_session(&self, session_key: &str) {
        if let Ok(Some(mut session)) = self.memory.get(session_key).await {
            session.clear();
            let _ = self.memory.save(&session).await;
        }
    }

    /// Get session message count
    pub async fn session_message_count(&self, session_key: &str) -> usize {
        self.memory
            .get(session_key)
            .await
            .ok()
            .flatten()
            .map(|s| s.message_count())
            .unwrap_or(0)
    }
}
