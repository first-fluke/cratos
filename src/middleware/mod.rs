//! Middleware module for Cratos HTTP server
//!
//! Provides:
//! - Authentication middleware (Bearer token / API key)
//! - Rate limiting middleware
//! - Scope-based authorization

pub mod auth;
pub mod rate_limit;

// Re-exports used by submodules via crate::middleware::auth / crate::middleware::rate_limit
