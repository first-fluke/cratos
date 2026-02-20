//! Persona Routing
//!
//! Maps Olympus OS persona names to existing agent IDs.
//!
//! # Examples
//!
//! ```text
//! @sindri implement the API   -> calls backend agent
//! @athena create a plan       -> calls pm agent
//! @cratos summarize status    -> handled by orchestrator
//! ```
//!
//! # Multi-Persona Routing
//!
//! Supports multiple personas in a single message:
//!
//! ```text
//! @nike @apollo 작업해줘           -> Parallel execution
//! @athena 계획 -> @sindri 구현      -> Pipeline execution (Phase 2)
//! @sindri @heimdall collaborate:    -> Collaborative execution (Phase 3)
//! ```

use crate::pantheon::{Domain, PersonaLoader, PersonaPreset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Execution mode for multi-persona routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Execute all personas in parallel, merge results
    #[default]
    Parallel,
    /// Execute personas in sequence, pass output to next input (Phase 2)
    Pipeline,
    /// Personas collaborate via A2A communication (Phase 3)
    Collaborative,
}

/// A single persona mention extracted from user input
#[derive(Debug, Clone)]
pub struct PersonaMention {
    /// Persona name (lowercase)
    pub name: String,
    /// Mapped agent ID
    pub agent_id: String,
    /// Instruction specific to this persona (for Pipeline mode)
    pub instruction: Option<String>,
}

/// Result of extracting multiple persona mentions from a message
#[derive(Debug, Clone)]
pub struct MultiPersonaExtraction {
    /// Extracted personas in order
    pub personas: Vec<PersonaMention>,
    /// Remaining text after removing persona mentions
    pub rest: String,
    /// Detected execution mode
    pub mode: ExecutionMode,
}

/// Persona to Agent mapping
#[derive(Debug, Clone)]
pub struct PersonaMapping {
    /// Persona name (lowercase) -> Agent ID
    name_to_agent: HashMap<String, String>,
    /// Agent ID -> Persona name
    agent_to_name: HashMap<String, String>,
    /// Loaded preset cache
    presets: HashMap<String, PersonaPreset>,
}

impl PersonaMapping {
    /// Create default mapping (hardcoded)
    #[must_use]
    pub fn default_mapping() -> Self {
        let mut name_to_agent = HashMap::new();
        let mut agent_to_name = HashMap::new();

        // Core 5 + Extended 9 mapping
        let mappings = [
            ("cratos", "orchestrator"),
            ("athena", "pm"),
            ("sindri", "backend"),
            ("heimdall", "qa"),
            ("mimir", "researcher"),
            ("odin", "po"),
            ("hestia", "hr"),
            ("norns", "ba"),
            ("apollo", "ux"),
            ("freya", "cs"),
            ("tyr", "legal"),
            ("nike", "marketing"),
            ("thor", "devops"),
            ("brok", "backend"),
        ];

        for (persona, agent) in mappings {
            name_to_agent.insert(persona.to_string(), agent.to_string());
            agent_to_name.insert(agent.to_string(), persona.to_string());
        }

        Self {
            name_to_agent,
            agent_to_name,
            presets: HashMap::new(),
        }
    }

    /// Create mapping by loading personas from TOML files
    pub fn from_loader(loader: &PersonaLoader) -> Self {
        let mut mapping = Self::default_mapping();

        if let Ok(presets) = loader.load_all() {
            for preset in presets {
                let persona_name = preset.persona.name.to_lowercase();
                let agent_id = preset.persona.domain.to_agent_id().to_string();

                mapping
                    .name_to_agent
                    .insert(persona_name.clone(), agent_id.clone());
                mapping.agent_to_name.insert(agent_id, persona_name.clone());

                mapping.presets.insert(persona_name, preset);
            }
        }

        mapping
    }

    /// Persona name -> Agent ID
    #[must_use]
    pub fn to_agent_id(&self, persona_name: &str) -> Option<&str> {
        self.name_to_agent
            .get(&persona_name.to_lowercase())
            .map(|s| s.as_str())
    }

    /// Agent ID -> Persona name
    #[must_use]
    pub fn to_persona_name(&self, agent_id: &str) -> Option<&str> {
        self.agent_to_name
            .get(&agent_id.to_lowercase())
            .map(|s| s.as_str())
    }

    /// Get persona preset
    #[must_use]
    pub fn get_preset(&self, persona_name: &str) -> Option<&PersonaPreset> {
        self.presets.get(&persona_name.to_lowercase())
    }

    /// Check if persona name exists
    #[must_use]
    pub fn is_persona(&self, name: &str) -> bool {
        self.name_to_agent.contains_key(&name.to_lowercase())
    }

    /// Check if agent ID is mapped to a persona
    #[must_use]
    pub fn has_agent(&self, agent_id: &str) -> bool {
        self.agent_to_name.contains_key(&agent_id.to_lowercase())
    }

    /// Return all persona names
    #[must_use]
    pub fn persona_names(&self) -> Vec<&str> {
        self.name_to_agent.keys().map(|s| s.as_str()).collect()
    }

    /// Generate system prompt (persona-based)
    #[must_use]
    pub fn get_system_prompt(&self, persona_name: &str, user_name: &str) -> Option<String> {
        self.get_preset(persona_name)
            .map(|preset| preset.to_system_prompt(user_name))
    }

    /// Format response (persona style)
    #[must_use]
    pub fn format_response(
        &self,
        persona_name: &str,
        content: &str,
        law_reference: Option<&str>,
    ) -> String {
        if let Some(preset) = self.get_preset(persona_name) {
            preset.format_response(content, law_reference)
        } else {
            // Default format if preset not loaded
            format!("[{persona_name}] {content}")
        }
    }
}

impl Default for PersonaMapping {
    fn default() -> Self {
        Self::default_mapping()
    }
}

/// Extract all persona mentions from a message.
///
/// Supports multiple execution modes:
/// - **Parallel**: `@nike @apollo 작업해줘` - all personas execute simultaneously
/// - **Pipeline**: `@athena 계획 -> @sindri 구현` - sequential, output chains to next (Phase 2)
/// - **Collaborative**: `@sindri @heimdall collaborate: API` - A2A communication (Phase 3)
///
/// # Arguments
/// * `message` - User message
/// * `mapping` - Persona mapping
///
/// # Returns
/// `MultiPersonaExtraction` with personas, rest text, and detected mode. None if no persona found.
pub fn extract_all_persona_mentions(
    message: &str,
    mapping: &PersonaMapping,
) -> Option<MultiPersonaExtraction> {
    // Detect execution mode
    let mode = detect_execution_mode(message);

    match mode {
        ExecutionMode::Pipeline => extract_pipeline_personas(message, mapping),
        ExecutionMode::Collaborative => extract_collaborative_personas(message, mapping),
        ExecutionMode::Parallel => extract_parallel_personas(message, mapping),
    }
}

/// Detect execution mode from message syntax
fn detect_execution_mode(message: &str) -> ExecutionMode {
    // Pipeline: contains "->" separator
    if message.contains("->") {
        return ExecutionMode::Pipeline;
    }

    // Collaborative: contains "collaborate:" prefix (case-insensitive)
    let lower = message.to_lowercase();
    if lower.contains("collaborate:") || lower.contains("협업:") {
        return ExecutionMode::Collaborative;
    }

    // Default: Parallel
    ExecutionMode::Parallel
}

/// Extract personas for Parallel mode
fn extract_parallel_personas(
    message: &str,
    mapping: &PersonaMapping,
) -> Option<MultiPersonaExtraction> {
    let mut personas = Vec::new();
    let mut rest_parts = Vec::new();
    let mut in_persona_prefix = true;

    for token in message.split_whitespace() {
        let cleaned = token
            .trim_start_matches('@')
            .trim_end_matches(&[',', '.', '!', '?', ':', ';'] as &[char]);
        let lower = cleaned.to_lowercase();

        if in_persona_prefix {
            if let Some(agent_id) = mapping.to_agent_id(&lower) {
                personas.push(PersonaMention {
                    name: lower.clone(),
                    agent_id: agent_id.to_string(),
                    instruction: None,
                });
                debug!(persona = %lower, agent_id = agent_id, "Parallel persona detected");
                continue;
            }
            // Once we hit a non-persona token, stop looking for more
            in_persona_prefix = false;
        }
        rest_parts.push(token);
    }

    if personas.is_empty() {
        return None;
    }

    Some(MultiPersonaExtraction {
        personas,
        rest: rest_parts.join(" "),
        mode: ExecutionMode::Parallel,
    })
}

/// Extract personas for Pipeline mode (Phase 2 - basic parsing only)
fn extract_pipeline_personas(
    message: &str,
    mapping: &PersonaMapping,
) -> Option<MultiPersonaExtraction> {
    let mut personas = Vec::new();

    // Split by "->" to get pipeline stages
    let stages: Vec<&str> = message.split("->").map(|s| s.trim()).collect();

    for stage in &stages {
        // Extract first token as persona
        let first_token = stage.split_whitespace().next().map(|t| {
            t.trim_start_matches('@')
                .trim_end_matches(&[',', '.'] as &[char])
        });

        if let Some(token) = first_token {
            let lower = token.to_lowercase();
            if let Some(agent_id) = mapping.to_agent_id(&lower) {
                let instruction = stage
                    .split_once(char::is_whitespace)
                    .map(|(_, rest)| rest.trim().to_string());

                personas.push(PersonaMention {
                    name: lower.clone(),
                    agent_id: agent_id.to_string(),
                    instruction,
                });
                debug!(persona = %lower, agent_id = agent_id, "Pipeline persona detected");
            }
        }
    }

    if personas.is_empty() {
        return None;
    }

    // For pipeline, rest is the entire message (instructions are per-stage)
    Some(MultiPersonaExtraction {
        personas,
        rest: message.to_string(),
        mode: ExecutionMode::Pipeline,
    })
}

/// Extract personas for Collaborative mode (Phase 3 - basic parsing only)
fn extract_collaborative_personas(
    message: &str,
    mapping: &PersonaMapping,
) -> Option<MultiPersonaExtraction> {
    let mut personas = Vec::new();

    // Find "collaborate:" or "협업:" and extract text after it
    let lower = message.to_lowercase();
    let collaborate_idx = lower.find("collaborate:").or_else(|| lower.find("협업:"));

    let (prefix, task) = if let Some(idx) = collaborate_idx {
        let collab_len = if lower[idx..].starts_with("collaborate:") {
            "collaborate:".len()
        } else {
            "협업:".len()
        };
        (&message[..idx], message[idx + collab_len..].trim())
    } else {
        (message, "")
    };

    // Extract personas from prefix
    for token in prefix.split_whitespace() {
        let cleaned = token
            .trim_start_matches('@')
            .trim_end_matches(&[',', '.', '!', '?', ':', ';'] as &[char]);
        let lower = cleaned.to_lowercase();

        if let Some(agent_id) = mapping.to_agent_id(&lower) {
            personas.push(PersonaMention {
                name: lower.clone(),
                agent_id: agent_id.to_string(),
                instruction: None,
            });
            debug!(persona = %lower, agent_id = agent_id, "Collaborative persona detected");
        }
    }

    if personas.is_empty() {
        return None;
    }

    Some(MultiPersonaExtraction {
        personas,
        rest: task.to_string(),
        mode: ExecutionMode::Collaborative,
    })
}

/// Extract persona mention from message (supports @mention, bare name, and aliases from TOML)
///
/// **Backward-compatible wrapper** around `extract_all_persona_mentions()`.
/// Returns only the first persona found.
///
/// # Arguments
/// * `message` - User message
/// * `mapping` - Persona mapping (includes aliases loaded from TOML)
///
/// # Returns
/// `(agent_id, rest_of_message)` tuple. None if no persona found.
pub fn extract_persona_mention(
    message: &str,
    mapping: &PersonaMapping,
) -> Option<(String, String)> {
    extract_all_persona_mentions(message, mapping)
        .filter(|e| !e.personas.is_empty())
        .map(|e| (e.personas[0].agent_id.clone(), e.rest))
}

/// Get agent ID by domain
#[must_use]
pub fn domain_to_agent_id(domain: Domain) -> &'static str {
    domain.to_agent_id()
}

#[cfg(test)]
mod tests;

