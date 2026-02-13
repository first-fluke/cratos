//! Gemini - Google Gemini API provider
//!
//! This module implements the Google Gemini provider using reqwest.

mod config;
mod convert;
mod provider;
mod schema;
mod security;
mod types;

#[cfg(test)]
mod tests;

// Re-export public API
pub use config::{GeminiAuth, GeminiConfig, DEFAULT_MODEL, MODELS};
pub use provider::GeminiProvider;
