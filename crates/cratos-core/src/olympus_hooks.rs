//! Olympus Hooks - Post-execution integration
//!
//! Connects the Orchestrator with Olympus OS subsystems:
//! - Law enforcement (validate responses)
//! - Chronicle auto-logging
//! - Auto-promotion checks

use crate::chronicles::ChronicleStore;
use crate::decrees::{EnforcementAction, EnforcerConfig, LawEnforcer};
use crate::error::Result;
use crate::pantheon::ActivePersonaState;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Configuration for Olympus hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OlympusConfig {
    /// Auto-log entries to chronicle after task execution
    #[serde(default = "default_true")]
    pub auto_chronicle: bool,
    /// Auto-check promotion eligibility after logging
    #[serde(default = "default_true")]
    pub auto_promotion: bool,
    /// Law enforcer configuration
    #[serde(default)]
    pub enforcer: EnforcerConfig,
}

fn default_true() -> bool {
    true
}

impl Default for OlympusConfig {
    fn default() -> Self {
        Self {
            auto_chronicle: true,
            auto_promotion: true,
            enforcer: EnforcerConfig::default(),
        }
    }
}

/// Summary of post-execution processing
#[derive(Debug, Default)]
pub struct PostExecutionSummary {
    /// Enforcement actions taken
    pub enforcement_actions: Vec<EnforcementAction>,
    /// Whether a chronicle entry was added
    pub chronicle_logged: bool,
    /// Whether a promotion occurred
    pub promoted: bool,
    /// New level after promotion (if any)
    pub new_level: Option<u8>,
}

/// Olympus OS hooks for post-execution processing
pub struct OlympusHooks {
    config: OlympusConfig,
    chronicle_store: ChronicleStore,
    enforcer: LawEnforcer,
    active_persona: ActivePersonaState,
}

impl OlympusHooks {
    /// Create new hooks
    #[must_use]
    pub fn new(config: OlympusConfig) -> Self {
        let chronicle_store = ChronicleStore::new();
        let enforcer = LawEnforcer::new(config.enforcer.clone(), ChronicleStore::new());
        let active_persona = ActivePersonaState::new();

        Self {
            config,
            chronicle_store,
            enforcer,
            active_persona,
        }
    }

    /// Run post-execution hooks
    ///
    /// This is designed to be fire-and-forget - failures are logged but don't
    /// propagate to the caller.
    pub fn post_execute(
        &self,
        persona: &str,
        response: &str,
        task_completed: bool,
    ) -> Result<PostExecutionSummary> {
        let mut summary = PostExecutionSummary::default();

        // 1. Law enforcement
        let violations = self
            .enforcer
            .validate_response(persona, response, task_completed);

        if !violations.is_empty() {
            debug!(
                persona = persona,
                violations = violations.len(),
                "Law violations detected"
            );
            match self.enforcer.enforce(&violations) {
                Ok(actions) => {
                    summary.enforcement_actions = actions;
                }
                Err(e) => {
                    warn!(error = %e, "Failed to enforce violations");
                }
            }
        }

        // 2. Auto-chronicle logging
        if self.config.auto_chronicle {
            let entry_text = if task_completed {
                format!("Task completed: {}", truncate_response(response, 100))
            } else {
                format!("Response: {}", truncate_response(response, 80))
            };

            match self.chronicle_store.load(persona) {
                Ok(Some(mut chronicle)) => {
                    chronicle.add_entry(&entry_text, None);
                    if let Err(e) = self.chronicle_store.save(&chronicle) {
                        warn!(error = %e, persona = persona, "Failed to save chronicle entry");
                    } else {
                        summary.chronicle_logged = true;

                        // 3. Auto-promotion check
                        if self.config.auto_promotion && chronicle.is_promotion_eligible() {
                            let old_level = chronicle.level;
                            if chronicle.promote() {
                                if let Err(e) = self.chronicle_store.save(&chronicle) {
                                    warn!(error = %e, "Failed to save promotion");
                                } else {
                                    summary.promoted = true;
                                    summary.new_level = Some(chronicle.level);
                                    debug!(
                                        persona = persona,
                                        old_level = old_level,
                                        new_level = chronicle.level,
                                        "Auto-promotion applied"
                                    );
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    // No chronicle yet - create one
                    let mut chronicle =
                        crate::chronicles::Chronicle::new(persona);
                    chronicle.add_entry(&entry_text, None);
                    if let Err(e) = self.chronicle_store.save(&chronicle) {
                        warn!(error = %e, persona = persona, "Failed to create chronicle");
                    } else {
                        summary.chronicle_logged = true;
                    }
                }
                Err(e) => {
                    warn!(error = %e, persona = persona, "Failed to load chronicle");
                }
            }
        }

        Ok(summary)
    }

    /// Get the currently active persona
    pub fn active_persona(&self) -> Option<String> {
        self.active_persona.load().unwrap_or(None)
    }
}

/// Truncate a response string for logging
fn truncate_response(s: &str, max_len: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() > max_len {
        let truncated = match first_line.char_indices().take_while(|(i, _)| *i < max_len).last() {
            Some((i, c)) => &first_line[..i + c.len_utf8()],
            None => "",
        };
        format!("{truncated}...")
    } else {
        first_line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_olympus_config_default() {
        let config = OlympusConfig::default();
        assert!(config.auto_chronicle);
        assert!(config.auto_promotion);
        assert!(config.enforcer.enabled);
    }

    #[test]
    fn test_truncate_response() {
        assert_eq!(truncate_response("hello", 10), "hello");
        assert_eq!(truncate_response("hello world!", 5), "hello...");
        assert_eq!(
            truncate_response("line1\nline2\nline3", 100),
            "line1"
        );
    }

    #[test]
    fn test_post_execution_summary_default() {
        let summary = PostExecutionSummary::default();
        assert!(!summary.chronicle_logged);
        assert!(!summary.promoted);
        assert!(summary.enforcement_actions.is_empty());
        assert!(summary.new_level.is_none());
    }
}
