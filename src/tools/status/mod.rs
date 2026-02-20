//! Status tool — lets the LLM query persona chronicles and skill metrics
//!
//! Registered as a built-in tool so the orchestrator can invoke it when a user
//! asks "how's sindri doing?" or "show me skill stats".

use cratos_core::chronicles::ChronicleStore;
use cratos_skills::SkillStore;
use cratos_tools::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

/// Target parameter values
const TARGET_PERSONA: &str = "persona";
const TARGET_SKILL: &str = "skill";
const TARGET_ALL: &str = "all";

/// The `status` tool for querying persona and skill metrics.
pub struct StatusTool {
    definition: ToolDefinition,
    skill_store: Arc<SkillStore>,
    chronicle_store: ChronicleStore,
}

impl StatusTool {
    /// Create a new status tool.
    pub fn new(skill_store: Arc<SkillStore>) -> Self {
        let definition = ToolDefinition::new(
            "status",
            "Query persona chronicles (ratings, judgments) and skill metrics (success rates, usage). \
             Use target=\"persona\" with optional name to query a specific persona, \
             target=\"skill\" with optional name for a specific skill, \
             or target=\"all\" for a combined overview.",
        )
        .with_parameters(json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "enum": [TARGET_PERSONA, TARGET_SKILL, TARGET_ALL],
                    "description": "What to query: persona chronicles, skill metrics, or both"
                },
                "name": {
                    "type": "string",
                    "description": "Optional specific persona or skill name"
                }
            },
            "required": ["target"]
        }))
        .with_risk_level(RiskLevel::Low)
        .with_category(ToolCategory::Utility);

        Self {
            definition,
            skill_store,
            chronicle_store: ChronicleStore::new(),
        }
    }

    /// Query persona chronicle(s)
    fn query_persona(&self, name: Option<&str>) -> serde_json::Value {
        match name {
            Some(persona_name) => match self.chronicle_store.load(persona_name) {
                Ok(Some(chronicle)) => {
                    json!({
                        "persona": chronicle.persona_name,
                        "level": chronicle.level,
                        "status": format!("{:?}", chronicle.status).to_lowercase(),
                        "rating": chronicle.rating,
                        "entries": chronicle.log.len(),
                        "quests_total": chronicle.quests.len(),
                        "quests_completed": chronicle.completed_quests(),
                        "judgments": chronicle.judgments.len(),
                        "promotion_eligible": chronicle.is_promotion_eligible(),
                        "rating_gap": chronicle.rating_gap(),
                        "entries_until_promotion": chronicle.entries_until_promotion(),
                    })
                }
                Ok(None) => {
                    json!({"error": format!("persona '{}' not found", persona_name)})
                }
                Err(e) => {
                    json!({"error": format!("failed to load persona: {}", e)})
                }
            },
            None => {
                // Return all personas
                match self.chronicle_store.load_all() {
                    Ok(chronicles) => {
                        let personas: Vec<serde_json::Value> = chronicles
                            .iter()
                            .map(|c| {
                                json!({
                                    "persona": c.persona_name,
                                    "level": c.level,
                                    "status": format!("{:?}", c.status).to_lowercase(),
                                    "rating": c.rating,
                                    "entries": c.log.len(),
                                    "promotion_eligible": c.is_promotion_eligible(),
                                })
                            })
                            .collect();
                        json!({"personas": personas})
                    }
                    Err(e) => json!({"error": format!("failed to load chronicles: {}", e)}),
                }
            }
        }
    }

    /// Query skill metric(s)
    async fn query_skill(&self, name: Option<&str>) -> serde_json::Value {
        match name {
            Some(skill_name) => match self.skill_store.get_skill_by_name(skill_name).await {
                Ok(Some(skill)) => {
                    let (total, successes) =
                        match self.skill_store.get_skill_execution_count(skill.id).await {
                            Ok(counts) => counts,
                            Err(e) => {
                                tracing::error!("Failed to get skill execution count: {}", e);
                                (0, 0)
                            }
                        };
                    json!({
                        "name": skill.name,
                        "status": skill.status.as_str(),
                        "category": skill.category.as_str(),
                        "origin": skill.origin.as_str(),
                        "success_rate": skill.metadata.success_rate,
                        "usage_count": skill.metadata.usage_count,
                        "executions_total": total,
                        "executions_succeeded": successes,
                        "avg_duration_ms": skill.metadata.avg_duration_ms,
                        "steps": skill.steps.len(),
                        "keywords": skill.trigger.keywords,
                    })
                }
                Ok(None) => json!({"error": format!("skill '{}' not found", skill_name)}),
                Err(e) => json!({"error": format!("failed to load skill: {}", e)}),
            },
            None => {
                // Return all skills
                match self.skill_store.list_skills().await {
                    Ok(skills) => {
                        let items: Vec<serde_json::Value> = skills
                            .iter()
                            .map(|s| {
                                json!({
                                    "name": s.name,
                                    "status": s.status.as_str(),
                                    "success_rate": s.metadata.success_rate,
                                    "usage_count": s.metadata.usage_count,
                                })
                            })
                            .collect();
                        json!({"skills": items})
                    }
                    Err(e) => json!({"error": format!("failed to list skills: {}", e)}),
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl Tool for StatusTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> cratos_tools::Result<ToolResult> {
        let start = Instant::now();

        let target = input
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or(TARGET_ALL);
        let name = input.get("name").and_then(|v| v.as_str());

        let output = match target {
            TARGET_PERSONA => self.query_persona(name),
            TARGET_SKILL => self.query_skill(name).await,
            TARGET_ALL => {
                let personas = self.query_persona(None);
                let skills = self.query_skill(None).await;
                json!({
                    "personas": personas.get("personas").cloned().unwrap_or_else(|| json!([])),
                    "skills": skills.get("skills").cloned().unwrap_or_else(|| json!([])),
                })
            }
            other => {
                json!({"error": format!("unknown target: '{}'. Use persona, skill, or all", other)})
            }
        };

        let duration = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(output, duration))
    }
}

#[cfg(test)]
mod tests;
