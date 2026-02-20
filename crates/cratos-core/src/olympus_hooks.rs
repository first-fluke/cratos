//! Olympus Hooks - Post-execution integration
//!
//! Connects the Orchestrator with Olympus OS subsystems:
//! - Law enforcement (validate responses)
//! - Chronicle auto-logging
//! - Auto-promotion checks
//! - Skill proficiency synchronization (Phase 7)

use crate::chronicles::ChronicleStore;
use crate::decrees::{EnforcementAction, EnforcerConfig, LawEnforcer};
use crate::error::Result;
use crate::pantheon::ActivePersonaState;
use cratos_skills::persona::OwnershipType;
use cratos_skills::PersonaSkillStore;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

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

/// Score awarded for successful task completion without violations
const REWARD_SCORE: f32 = 4.0;

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

                    // Reward: successful task completion without violations
                    if violations.is_empty() && task_completed {
                        chronicle.add_judgment(
                            "Cratos",
                            "Task completed successfully",
                            Some(REWARD_SCORE),
                        );
                    }

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
                    let mut chronicle = crate::chronicles::Chronicle::new(persona);
                    chronicle.add_entry(&entry_text, None);

                    if violations.is_empty() && task_completed {
                        chronicle.add_judgment(
                            "Cratos",
                            "Task completed successfully",
                            Some(REWARD_SCORE),
                        );
                    }

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

    /// Synchronize skill proficiency from PersonaSkillStore to Chronicle (Phase 7)
    ///
    /// This method fetches the persona's skill bindings and updates the Chronicle with:
    /// - `skill_proficiency`: Map of skill_name -> success_rate
    /// - `auto_assigned_skills`: List of skills that were auto-assigned
    ///
    /// Call this periodically or after significant skill metric changes.
    pub async fn sync_skill_proficiency(
        &self,
        persona: &str,
        store: &PersonaSkillStore,
    ) -> Result<SyncSkillProficiencyResult> {
        let mut result = SyncSkillProficiencyResult::default();

        // Get all persona skills from the store
        let skills = store.get_persona_skills(persona).await.map_err(|e| {
            crate::error::Error::Internal(format!("Failed to get persona skills: {e}"))
        })?;

        if skills.is_empty() {
            debug!(persona = %persona, "No skills found for persona");
            return Ok(result);
        }

        // Load or create chronicle
        let mut chronicle = match self.chronicle_store.load(persona)? {
            Some(c) => c,
            None => {
                debug!(persona = %persona, "Creating new chronicle for skill sync");
                crate::chronicles::Chronicle::new(persona)
            }
        };

        // Update skill proficiency map
        for skill in &skills {
            // Only include skills with meaningful usage (at least 3 uses)
            if skill.usage_count >= 3 {
                let old_rate = chronicle.skill_proficiency.get(&skill.skill_name).copied();
                chronicle.update_skill_proficiency(&skill.skill_name, skill.success_rate);

                if old_rate != Some(skill.success_rate) {
                    result.skills_updated += 1;
                }
            }

            // Track auto-assigned skills
            if skill.ownership_type == OwnershipType::AutoAssigned
                && !chronicle.auto_assigned_skills.contains(&skill.skill_name)
            {
                chronicle.record_auto_assignment(&skill.skill_name);
                result.new_auto_assignments += 1;
                info!(
                    persona = %persona,
                    skill = %skill.skill_name,
                    "Recorded auto-assignment in chronicle"
                );
            }
        }

        // Save chronicle
        self.chronicle_store.save(&chronicle)?;
        result.success = true;

        debug!(
            persona = %persona,
            skills_updated = %result.skills_updated,
            new_auto_assignments = %result.new_auto_assignments,
            "Skill proficiency sync completed"
        );

        Ok(result)
    }

    /// Get a reference to the chronicle store
    pub fn chronicle_store(&self) -> &ChronicleStore {
        &self.chronicle_store
    }
}

/// Result of skill proficiency synchronization
#[derive(Debug, Default)]
pub struct SyncSkillProficiencyResult {
    /// Whether the sync was successful
    pub success: bool,
    /// Number of skills whose proficiency was updated
    pub skills_updated: usize,
    /// Number of newly recorded auto-assignments
    pub new_auto_assignments: usize,
}

/// Truncate a response string for logging
fn truncate_response(s: &str, max_len: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() > max_len {
        let truncated = match first_line
            .char_indices()
            .take_while(|(i, _)| *i < max_len)
            .last()
        {
            Some((i, c)) => &first_line[..i + c.len_utf8()],
            None => "",
        };
        format!("{truncated}...")
    } else {
        first_line.to_string()
    }
}

#[cfg(test)]
mod tests;

