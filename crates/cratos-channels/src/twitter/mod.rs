//! Twitter - API v2 adapter
//!
//! This module provides a basic Twitter adapter using the API v2.

/// Twitter API v2 adapter implementation.
pub mod adapter;
/// Twitter API credentials and configuration.
pub mod config;

pub use adapter::TwitterAdapter;
pub use config::TwitterConfig;
