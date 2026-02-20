//! Anthropic - Claude API provider
//!
//! This module implements the Anthropic Claude provider using reqwest.

/// Message conversion utilities
pub mod convert;
/// Provider implementation
pub mod provider;
/// Security and sanitization utilities
pub mod security;
/// API types and configuration
pub mod types;

#[cfg(test)]
mod tests;

pub use provider::AnthropicProvider;
pub use types::{AnthropicConfig, DEFAULT_MODEL, MODELS};
