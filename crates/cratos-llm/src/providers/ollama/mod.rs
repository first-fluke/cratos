//! Ollama - Local Ollama API provider
//!
//! This module implements the Ollama provider for local LLM inference.
//! Ollama runs models locally and provides an OpenAI-compatible API.

#![allow(missing_docs)]

pub mod convert;
pub mod provider;
pub mod security;
pub mod types;

#[cfg(test)]
mod tests;

pub use provider::OllamaProvider;
pub use types::{OllamaConfig, DEFAULT_MODEL, SUGGESTED_MODELS};
