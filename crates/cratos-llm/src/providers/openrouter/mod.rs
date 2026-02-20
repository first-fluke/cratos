//! OpenRouter - Multi-provider LLM Gateway
//!
//! This module implements the OpenRouter provider which gives access to
//! 100+ models through a single API, including free tier models.

/// OpenRouter provider implementation
pub mod provider;
/// OpenRouter API and configuration types
pub mod types;

#[cfg(test)]
mod tests;

pub use provider::OpenRouterProvider;
pub use types::{OpenRouterConfig, BASE_URL, DEFAULT_MODEL, MODELS};
