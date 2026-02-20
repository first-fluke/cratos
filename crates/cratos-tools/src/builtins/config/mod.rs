//! Configuration Tool - LLM-driven structured input

pub mod handler;
pub mod tool;
pub mod types;
pub mod wol;

#[cfg(test)]
mod tests;

pub use tool::ConfigTool;
pub use types::{ConfigAction, ConfigInput, ConfigTarget};
