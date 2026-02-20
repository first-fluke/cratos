use cratos_core::pantheon::PersonaLoader;
use cratos_tools::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use serde_json::json;
use std::time::Instant;

/// Detailed persona information query tool
pub struct PersonaTool {
    definition: ToolDefinition,
    loader: PersonaLoader,
}

impl PersonaTool {
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "persona_info",
            "Get detailed information about available pantheon personas. \
             Use action='list' to see all available personas, or action='info' \
             with a 'name' to get specific details (domains, skills, instructions).",
        )
        .with_category(ToolCategory::Utility)
        .with_risk_level(RiskLevel::Low)
        .with_parameters(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "info"],
                    "description": "Action to perform: 'list' returns all persona names/domains, 'info' returns full details"
                },
                "name": {
                    "type": "string",
                    "description": "The specific persona name to get details for (required if action='info')"
                }
            },
            "required": ["action"]
        }));

        Self {
            definition,
            loader: PersonaLoader::new(),
        }
    }
}

impl Default for PersonaTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for PersonaTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> cratos_tools::Result<ToolResult> {
        let start = Instant::now();

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");

        match action {
            "list" => {
                let presets = match self.loader.load_all() {
                    Ok(p) => p,
                    Err(e) => {
                        return Ok(ToolResult::success(
                            json!({"error": format!("Failed to load personas: {}", e)}),
                            0,
                        ))
                    }
                };

                let list: Vec<_> = presets
                    .into_iter()
                    .map(|p| {
                        json!({
                            "name": p.persona.name,
                            "title": p.persona.title,
                            "domain": p.persona.domain,
                            "level": p.level.level
                        })
                    })
                    .collect();

                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult::success(
                    json!({
                        "count": list.len(),
                        "personas": list
                    }),
                    duration_ms,
                ))
            }
            "info" => {
                let name = match input.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return Ok(ToolResult::success(
                            json!({"error": "Missing 'name' argument for 'info' action"}),
                            0,
                        ))
                    }
                };

                let preset = match self.loader.load(name) {
                    Ok(p) => p,
                    Err(e) => {
                        return Ok(ToolResult::success(
                            json!({"error": format!("Failed to load persona '{}': {}", name, e)}),
                            0,
                        ))
                    }
                };

                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult::success(
                    json!({
                        "persona": preset.persona,
                        "traits": preset.traits,
                        "principles": preset.principles,
                        "skills": preset.skills,
                        "instructions": preset.instructions,
                        "level": preset.level
                    }),
                    duration_ms,
                ))
            }
            _ => Ok(ToolResult::success(
                json!({"error": format!("Invalid action '{}'", action)}),
                0,
            )),
        }
    }
}

#[cfg(test)]
mod tests;
