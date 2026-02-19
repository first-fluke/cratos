//! Twitter - API v2 adapter
//!
//! This module provides a basic Twitter adapter using the API v2.

pub mod adapter;
pub mod config;

pub use adapter::TwitterAdapter;
pub use config::TwitterConfig;
