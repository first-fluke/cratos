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

use crate::pantheon::{Domain, PersonaLoader, PersonaPreset};
use std::collections::HashMap;
use tracing::debug;

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

/// Extract persona @mention from message
///
/// # Arguments
/// * `message` - User message
/// * `mapping` - Persona mapping
///
/// # Returns
/// `(agent_id, rest_of_message)` tuple. None if no persona found.
pub fn extract_persona_mention(
    message: &str,
    mapping: &PersonaMapping,
) -> Option<(String, String)> {
    // Find @name pattern
    if !message.starts_with('@') {
        return None;
    }

    // Mention is until first whitespace
    let parts: Vec<&str> = message.splitn(2, char::is_whitespace).collect();
    if parts.is_empty() {
        return None;
    }

    let mention = parts[0].trim_start_matches('@').to_lowercase();
    let rest = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();

    // Check persona mapping
    if let Some(agent_id) = mapping.to_agent_id(&mention) {
        debug!(
            persona = mention,
            agent_id = agent_id,
            "Persona mention detected"
        );
        return Some((agent_id.to_string(), rest));
    }

    None
}

/// Get agent ID by domain
#[must_use]
pub fn domain_to_agent_id(domain: Domain) -> &'static str {
    domain.to_agent_id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mapping() {
        let mapping = PersonaMapping::default_mapping();

        assert_eq!(mapping.to_agent_id("sindri"), Some("backend"));
        assert_eq!(mapping.to_agent_id("athena"), Some("pm"));
        assert_eq!(mapping.to_agent_id("heimdall"), Some("qa"));
        assert_eq!(mapping.to_agent_id("mimir"), Some("researcher"));
        assert_eq!(mapping.to_agent_id("cratos"), Some("orchestrator"));
    }

    #[test]
    fn test_reverse_mapping() {
        let mapping = PersonaMapping::default_mapping();

        // "backend" maps to "brok" (last inserted wins in HashMap)
        let backend_persona = mapping.to_persona_name("backend");
        assert!(backend_persona == Some("sindri") || backend_persona == Some("brok"));
        assert_eq!(mapping.to_persona_name("pm"), Some("athena"));
        assert_eq!(mapping.to_persona_name("qa"), Some("heimdall"));
        assert_eq!(mapping.to_persona_name("po"), Some("odin"));
        assert_eq!(mapping.to_persona_name("devops"), Some("thor"));
    }

    #[test]
    fn test_case_insensitive() {
        let mapping = PersonaMapping::default_mapping();

        assert_eq!(mapping.to_agent_id("SINDRI"), Some("backend"));
        assert_eq!(mapping.to_agent_id("Athena"), Some("pm"));
    }

    #[test]
    fn test_is_persona() {
        let mapping = PersonaMapping::default_mapping();

        assert!(mapping.is_persona("sindri"));
        assert!(mapping.is_persona("ATHENA"));
        assert!(!mapping.is_persona("unknown"));
    }

    #[test]
    fn test_persona_names() {
        let mapping = PersonaMapping::default_mapping();
        let names = mapping.persona_names();

        assert!(names.contains(&"sindri"));
        assert!(names.contains(&"athena"));
        assert!(names.contains(&"cratos"));
    }

    #[test]
    fn test_extract_persona_mention() {
        let mapping = PersonaMapping::default_mapping();

        let result = extract_persona_mention("@sindri implement the API", &mapping);
        assert!(result.is_some());
        let (agent_id, rest) = result.unwrap();
        assert_eq!(agent_id, "backend");
        assert_eq!(rest, "implement the API");
    }

    #[test]
    fn test_extract_persona_mention_no_match() {
        let mapping = PersonaMapping::default_mapping();

        // Unknown persona
        let result = extract_persona_mention("@unknown do something", &mapping);
        assert!(result.is_none());

        // Does not start with @
        let result = extract_persona_mention("sindri do something", &mapping);
        assert!(result.is_none());
    }

    #[test]
    fn test_format_response_without_preset() {
        let mapping = PersonaMapping::default_mapping();

        let response = mapping.format_response("sindri", "task completed", None);
        // Default format since preset not loaded
        assert_eq!(response, "[sindri] task completed");
    }

    #[test]
    fn test_domain_to_agent_id() {
        assert_eq!(domain_to_agent_id(Domain::Dev), "backend");
        assert_eq!(domain_to_agent_id(Domain::Pm), "pm");
        assert_eq!(domain_to_agent_id(Domain::Qa), "qa");
        assert_eq!(domain_to_agent_id(Domain::Researcher), "researcher");
        assert_eq!(domain_to_agent_id(Domain::Orchestrator), "orchestrator");
    }
}
