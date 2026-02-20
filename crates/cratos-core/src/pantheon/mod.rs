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
mod tests;

