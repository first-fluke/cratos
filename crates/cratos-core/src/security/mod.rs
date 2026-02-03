//! Security Module - Security utilities for Cratos
//!
//! This module provides security features:
//! - Prompt injection detection and prevention
//! - Input sanitization
//! - Output validation

pub mod injection;

pub use injection::{
    sanitize_input, validate_tool_output, InjectionDetector, InjectionError, InjectionPattern,
    SecurityConfig, ThreatLevel,
};
