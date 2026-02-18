//! Server module for Cratos
//!
//! Contains the main server initialization and runtime logic.
//!
//! # Module Structure
//!
//! - `config`: Configuration structures for all server components
//! - `adapters`: Embedding adapters for search and skills
//! - `loader`: Configuration loading from files and environment
//! - `providers`: LLM provider resolution and registration
//! - `validation`: Production configuration validation
//! - `cli`: CLI-specific orchestrator builder
//! - `init`: Main server initialization and run loop

mod adapters;
mod cli;
pub mod config;
mod init;
mod loader;
mod providers;
mod task_handler;
mod validation;

// Re-export public API
pub use cli::build_orchestrator_for_cli;
pub use init::run;
pub use loader::load_config;
pub use providers::resolve_llm_provider;
