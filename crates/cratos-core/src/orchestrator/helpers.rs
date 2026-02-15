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

    /// Update persona chronicle with new activity
    pub async fn update_persona_chronicle(
        &self,
        persona_name: &str,
        content: &str,
        execution_id: Uuid,
    ) {
        if let Some(store) = &self.chronicle_store {
            // Ignore "cratos" and system personas
            if persona_name == "cratos" || persona_name == "user" || persona_name == "system" {
                return;
            }

            match store.load(persona_name) {
                Ok(Some(mut chronicle)) => {
                    // 1. Add general activity entry
                    let entry_text = if content.len() > 100 {
                        format!("{}...", &content[..100])
                    } else {
                        content.to_string()
                    };
                    chronicle.add_entry(&format!("Excuted task: {}", entry_text), None);

                    // 2. Simple heuristic for Quest detection (can be improved with LLM later)
                    // If content starts with "Quest:" or contains "New Quest:", add it
                    if content.contains("Quest:") || content.contains("New Quest:") {
                        let lines: Vec<&str> = content.lines().collect();
                        for line in lines {
                            if let Some(quest_desc) = line
                                .strip_prefix("Quest:")
                                .or_else(|| line.strip_prefix("- Quest:"))
                            {
                                chronicle.add_quest(quest_desc.trim());
                            }
                        }
                    }

                    // 3. Simple heuristic for Quest Completion
                    // If content contains "Quest Completed:" mark matching quest as done
                    if content.contains("Quest Completed:") || content.contains("Completed Quest:")
                    {
                        let lines: Vec<&str> = content.lines().collect();
                        for line in lines {
                            if let Some(quest_desc) = line
                                .strip_prefix("Quest Completed:")
                                .or_else(|| line.strip_prefix("- Quest Completed:"))
                            {
                                let target = quest_desc.trim();
                                if let Some(idx) = chronicle
                                    .quests
                                    .iter()
                                    .position(|q| q.description.contains(target) && !q.completed)
                                {
                                    chronicle.complete_quest(idx);
                                }
                            }
                        }
                    }

                    if let Err(e) = store.save(&chronicle) {
                        warn!(
                            persona = %persona_name,
                            error = %e,
                            "Failed to save chronicle update"
                        );
                    } else {
                        use tracing::debug;
                        debug!(
                            persona = %persona_name,
                            execution_id = %execution_id,
                            "Chronicle updated"
                        );
                    }
                }
                Ok(None) => {
                    // No chronicle found - possibly create one? or just ignore.
                    // For now, logging warning
                    warn!(persona = %persona_name, "Chronicle not found for update");
                }
                Err(e) => {
                    warn!(
                        persona = %persona_name,
                        error = %e,
                        "Failed to load chronicle for update"
                    );
                }
            }
        }
    }
}
