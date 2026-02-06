//! Pantheon - Persona Preset System
//!
//! The WHO layer of Olympus OS: Agent identity definitions
//!
//! # Overview
//!
//! Pantheon is a system that defines and loads AI agent personas (identities)
//! from TOML files.
//!
//! # Example
//!
//! ```rust,ignore
//! use cratos_core::pantheon::{PersonaLoader, PersonaPreset};
//!
//! let loader = PersonaLoader::new();
//! let sindri = loader.load("sindri")?;
//! println!("Loaded: {} - {}", sindri.persona.name, sindri.persona.title);
//! ```

#![forbid(unsafe_code)]

mod active_persona;
mod domain;
mod loader;
mod preset;

pub use active_persona::ActivePersonaState;
pub use domain::Domain;
pub use loader::PersonaLoader;
pub use preset::{
    PersonaInfo, PersonaLevel, PersonaPreset, PersonaPrinciples, PersonaSkills, PersonaTraits,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_display() {
        assert_eq!(Domain::Dev.as_str(), "DEV");
        assert_eq!(Domain::Pm.as_str(), "PM");
        assert_eq!(Domain::Orchestrator.as_str(), "ORCHESTRATOR");
    }

    #[test]
    fn test_persona_level_default() {
        let level = PersonaLevel::default();
        assert_eq!(level.level, 1);
        assert_eq!(level.title, "Mortal");
    }

    #[test]
    fn test_persona_level_supreme() {
        let level = PersonaLevel::supreme();
        assert_eq!(level.level, PersonaLevel::SUPREME_LEVEL);
        assert_eq!(level.title, "Supreme");
    }
}
