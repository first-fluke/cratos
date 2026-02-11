//! Security Module - Security utilities for Cratos
//!
//! This module provides security features:
//! - Prompt injection detection and prevention
//! - Input sanitization
//! - Output validation

#[allow(missing_docs)]
pub mod audit;
pub mod injection;

pub use audit::{run_audit, AuditFinding, AuditInput, AuditReport, AuditSummary, Severity};
pub use injection::{
    sanitize_input, validate_tool_output, InjectionDetector, InjectionError, InjectionPattern,
    SecurityConfig, ThreatLevel,
};
