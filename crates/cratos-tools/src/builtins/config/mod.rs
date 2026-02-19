//! Configuration Tool - LLM-driven structured input

pub mod types;
pub mod wol;
pub mod handler;
pub mod tool;

#[cfg(test)]
mod tests;

pub use types::{ConfigAction, ConfigInput, ConfigTarget};
pub use tool::ConfigTool;
