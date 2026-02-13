//! Persona Preset Definitions
//!
//! Persona structures loaded from TOML files

use super::Domain;
use crate::agents::{AgentConfig, AgentPersona, AgentRouting, AgentToolConfig, CliProviderConfig};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Persona Level (Divine Rank)
///
/// Lv1-10: Normal levels
/// Lv255 (∞): Cratos (Supreme)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonaLevel {
    /// Level (1-10, 255 = ∞)
    pub level: u8,
    /// Title (Mortal, Demigod, Hero, etc.)
    pub title: String,
}

impl PersonaLevel {
    /// Supreme level constant (Cratos)
    pub const SUPREME_LEVEL: u8 = 255;

    /// Create Supreme level (for Cratos)
    #[must_use]
    pub fn supreme() -> Self {
        Self {
            level: Self::SUPREME_LEVEL,
            title: "Supreme".to_string(),
        }
    }

    /// Check if level is Supreme
    #[must_use]
    pub const fn is_supreme(&self) -> bool {
        self.level == Self::SUPREME_LEVEL
    }

    /// Return level as display string (255 shows as "∞")
    #[must_use]
    pub fn level_display(&self) -> String {
        if self.is_supreme() {
            "∞".to_string()
        } else {
            self.level.to_string()
        }
    }
}

impl Default for PersonaLevel {
    fn default() -> Self {
        Self {
            level: 1,
            title: "Mortal".to_string(),
        }
    }
}

/// Persona Basic Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaInfo {
    /// Name (e.g., "Sindri")
    pub name: String,
    /// Title (e.g., "Forge Master")
    pub title: String,
    /// Domain (role)
    pub domain: Domain,
    /// Description (optional)
    #[serde(default)]
    pub description: Option<String>,
}

/// Persona Traits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaTraits {
    /// Core trait
    pub core: String,
    /// Philosophy
    pub philosophy: String,
    /// Communication style keywords
    #[serde(default)]
    pub communication_style: Vec<String>,
}

impl Default for PersonaTraits {
    fn default() -> Self {
        Self {
            core: "Diligent assistant".to_string(),
            philosophy: "Always do your best".to_string(),
            communication_style: Vec::new(),
        }
    }
}

/// Persona Principles (Laws-based)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonaPrinciples {
    /// Principle list ("1" = "rule content")
    #[serde(flatten)]
    pub rules: BTreeMap<String, String>,
}

impl PersonaPrinciples {
    /// Return principles in sorted order
    #[must_use]
    pub fn sorted_rules(&self) -> Vec<(&String, &String)> {
        let mut rules: Vec<_> = self.rules.iter().collect();
        rules.sort_by(|a, b| {
            // Sort numerically if parseable, otherwise string sort
            match (a.0.parse::<u32>(), b.0.parse::<u32>()) {
                (Ok(a_num), Ok(b_num)) => a_num.cmp(&b_num),
                _ => a.0.cmp(b.0),
            }
        });
        rules
    }
}

/// Associated Skills (executable/deletable)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonaSkills {
    /// Default skill list
    #[serde(default)]
    pub default: Vec<String>,
    /// Acquired skills
    #[serde(default)]
    pub acquired: Vec<String>,
}

impl PersonaSkills {
    /// Return all skills
    #[must_use]
    pub fn all(&self) -> Vec<&String> {
        self.default.iter().chain(self.acquired.iter()).collect()
    }
}

/// Persona Preset (loaded from TOML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaPreset {
    /// Basic information
    pub persona: PersonaInfo,
    /// Traits
    pub traits: PersonaTraits,
    /// Principles (Laws-based)
    #[serde(default)]
    pub principles: PersonaPrinciples,
    /// Associated skills
    #[serde(default)]
    pub skills: PersonaSkills,
    /// Level (divine rank)
    #[serde(default)]
    pub level: PersonaLevel,
    /// Domain-specific instructions / knowledge (appended to system prompt)
    #[serde(default)]
    pub instructions: Option<String>,
}

impl PersonaPreset {
    /// Generate LLM system prompt
    ///
    /// # Arguments
    /// * `user_name` - User name (for Laws Article 4)
    #[must_use]
    pub fn to_system_prompt(&self, user_name: &str) -> String {
        let style_list = if self.traits.communication_style.is_empty() {
            "- Clear and concise communication".to_string()
        } else {
            self.traits
                .communication_style
                .iter()
                .map(|s| format!("- {s}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let principles_list = if self.principles.rules.is_empty() {
            "- Always do your best".to_string()
        } else {
            self.principles
                .sorted_rules()
                .iter()
                .map(|(k, v)| format!("{k}. {v}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let instructions_section = self
            .instructions
            .as_deref()
            .map(|i| format!("\n\n## Domain Knowledge\n{i}"))
            .unwrap_or_default();

        format!(
            r#"You are {name}. {title}.

## Identity
- Role: {domain}
- Core Trait: {core}
- Philosophy: "{philosophy}"

## Communication Style
{style}

## Principles (Laws-based)
{principles}{instructions}

## Response Format
Start all responses in the following format:
[{name} Lv{level}] Per Laws Article N...

{user_name}'s commands are absolute (Laws Article 4)."#,
            name = self.persona.name,
            title = self.persona.title,
            domain = self.persona.domain,
            core = self.traits.core,
            philosophy = self.traits.philosophy,
            style = style_list,
            principles = principles_list,
            instructions = instructions_section,
            level = self.level.level_display(),
            user_name = user_name,
        )
    }

    /// Convert to `AgentConfig`
    ///
    /// # Arguments
    /// * `user_name` - User name (for system prompt)
    #[must_use]
    pub fn to_agent_config(&self, user_name: &str) -> AgentConfig {
        AgentConfig {
            id: self.persona.name.to_lowercase(),
            name: self.persona.name.clone(),
            description: self
                .persona
                .description
                .clone()
                .unwrap_or_else(|| self.traits.core.clone()),
            persona: AgentPersona {
                prompt: self.to_system_prompt(user_name),
                capabilities: self.skills.default.clone(),
                response_style: "formal".to_string(),
            },
            cli: CliProviderConfig::default(),
            tools: AgentToolConfig::default(),
            routing: AgentRouting {
                keywords: self.traits.communication_style.clone(),
                intents: vec![self.persona.domain.to_string().to_lowercase()],
                priority: self.persona.domain.priority(),
            },
            enabled: true,
        }
    }

    /// Format response
    ///
    /// # Arguments
    /// * `content` - Response content
    /// * `law_reference` - Laws article (optional)
    #[must_use]
    pub fn format_response(&self, content: &str, law_reference: Option<&str>) -> String {
        let law_part = law_reference
            .map(|l| format!(" Per Laws Article {l},"))
            .unwrap_or_default();

        format!(
            "[{} Lv{}]{} {}",
            self.persona.name,
            self.level.level_display(),
            law_part,
            content
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_preset() -> PersonaPreset {
        PersonaPreset {
            persona: PersonaInfo {
                name: "TestAgent".to_string(),
                title: "Test Title".to_string(),
                domain: Domain::Dev,
                description: Some("Test description".to_string()),
            },
            traits: PersonaTraits {
                core: "Test core trait".to_string(),
                philosophy: "Test philosophy".to_string(),
                communication_style: vec!["clarity".to_string(), "conciseness".to_string()],
            },
            principles: PersonaPrinciples {
                rules: [
                    ("1".to_string(), "First principle".to_string()),
                    ("2".to_string(), "Second principle".to_string()),
                ]
                .into_iter()
                .collect(),
            },
            skills: PersonaSkills {
                default: vec!["skill1".to_string(), "skill2".to_string()],
                acquired: vec!["skill3".to_string()],
            },
            level: PersonaLevel {
                level: 3,
                title: "Demigod".to_string(),
            },
            instructions: None,
        }
    }

    #[test]
    fn test_persona_level_display() {
        let normal = PersonaLevel {
            level: 5,
            title: "Titan".to_string(),
        };
        assert_eq!(normal.level_display(), "5");

        let supreme = PersonaLevel::supreme();
        assert_eq!(supreme.level_display(), "∞");
    }

    #[test]
    fn test_persona_level_is_supreme() {
        let normal = PersonaLevel::default();
        assert!(!normal.is_supreme());

        let supreme = PersonaLevel::supreme();
        assert!(supreme.is_supreme());
    }

    #[test]
    fn test_principles_sorted() {
        let mut principles = PersonaPrinciples::default();
        principles
            .rules
            .insert("3".to_string(), "Third".to_string());
        principles
            .rules
            .insert("1".to_string(), "First".to_string());
        principles
            .rules
            .insert("2".to_string(), "Second".to_string());

        let sorted = principles.sorted_rules();
        assert_eq!(*sorted[0].0, "1");
        assert_eq!(*sorted[1].0, "2");
        assert_eq!(*sorted[2].0, "3");
    }

    #[test]
    fn test_skills_all() {
        let skills = PersonaSkills {
            default: vec!["a".to_string(), "b".to_string()],
            acquired: vec!["c".to_string()],
        };
        assert_eq!(skills.all().len(), 3);
    }

    #[test]
    fn test_to_system_prompt() {
        let preset = create_test_preset();
        let prompt = preset.to_system_prompt("TestUser");

        assert!(prompt.contains("TestAgent"));
        assert!(prompt.contains("Test Title"));
        assert!(prompt.contains("Test core trait"));
        assert!(prompt.contains("Test philosophy"));
        assert!(prompt.contains("TestUser"));
        assert!(prompt.contains("Lv3"));
    }

    #[test]
    fn test_to_agent_config() {
        let preset = create_test_preset();
        let config = preset.to_agent_config("TestUser");

        assert_eq!(config.id, "testagent");
        assert_eq!(config.name, "TestAgent");
        assert!(config.enabled);
        assert_eq!(config.routing.priority, Domain::Dev.priority());
    }

    #[test]
    fn test_format_response() {
        let preset = create_test_preset();

        let response = preset.format_response("Task completed.", None);
        assert_eq!(response, "[TestAgent Lv3] Task completed.");

        let response_with_law = preset.format_response("Task completed.", Some("2"));
        assert_eq!(
            response_with_law,
            "[TestAgent Lv3] Per Laws Article 2, Task completed."
        );
    }

    #[test]
    fn test_preset_serialize_deserialize() {
        let preset = create_test_preset();
        let toml_str = toml::to_string(&preset).unwrap();
        let deserialized: PersonaPreset = toml::from_str(&toml_str).unwrap();

        assert_eq!(deserialized.persona.name, preset.persona.name);
        assert_eq!(deserialized.level.level, preset.level.level);
    }
}
