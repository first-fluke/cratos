//! Routing logic for persona and skill selection
//!
//! Contains helper methods for routing requests to the appropriate skill.

use super::core::Orchestrator;
use tracing::{debug, info};
use uuid::Uuid;

/// Result of skill routing
pub struct SkillRoute {
    /// Skill hint to append to system prompt
    pub skill_hint: Option<String>,
    /// Matched skill ID
    pub skill_id: Option<Uuid>,
}

impl Orchestrator {
    /// Route the request to a skill with persona proficiency bonus
    pub(super) async fn route_to_skill(&self, text: &str, effective_persona: &str) -> SkillRoute {
        if let Some(router) = &self.skill_router {
            match router.route_best(text).await {
                Some(mut m) => {
                    // Apply persona skill proficiency bonus
                    if let Some(store) = &self.persona_skill_store {
                        let config = cratos_skills::AutoAssignmentConfig::default();
                        if let Ok(proficiency_map) =
                            store.get_skill_proficiency_map(effective_persona).await
                        {
                            if let Some(&success_rate) = proficiency_map.get(&m.skill_name) {
                                if success_rate >= config.proficiency_threshold {
                                    let old_score = m.score;
                                    m.score = (m.score + config.persona_skill_bonus).min(1.0);
                                    debug!(
                                        persona = %effective_persona,
                                        skill = %m.skill_name,
                                        old_score = %old_score,
                                        new_score = %m.score,
                                        success_rate = %success_rate,
                                        "Applied persona skill proficiency bonus"
                                    );
                                }
                            }
                        }
                    }

                    // Only accept if score exceeds threshold (after bonus)
                    if m.score > 0.7 {
                        info!(
                            skill = %m.skill_name,
                            skill_id = %m.skill_id,
                            score = %m.score,
                            persona = %effective_persona,
                            "Skill match found"
                        );
                        SkillRoute {
                            skill_hint: Some(format!(
                                "\n## Matched Skill: {}\n{}",
                                m.skill_name, m.description
                            )),
                            skill_id: Some(m.skill_id),
                        }
                    } else {
                        SkillRoute {
                            skill_hint: None,
                            skill_id: None,
                        }
                    }
                }
                None => SkillRoute {
                    skill_hint: None,
                    skill_id: None,
                },
            }
        } else {
            SkillRoute {
                skill_hint: None,
                skill_id: None,
            }
        }
    }

    /// Combine system prompt overrides
    pub(super) fn combine_system_prompts(
        &self,
        input_override: Option<&str>,
        persona_prompt: Option<String>,
        skill_hint: Option<String>,
    ) -> Option<String> {
        if let Some(override_prompt) = input_override {
            Some(override_prompt.to_string())
        } else {
            match (persona_prompt, skill_hint) {
                (Some(p), Some(s)) => Some(format!("{}{}", p, s)),
                (Some(p), None) => Some(p),
                (None, Some(s)) => Some(format!("{}{}", self.planner.config().system_prompt, s)),
                (None, None) => None,
            }
        }
    }
}
