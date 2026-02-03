//! Cratos Skills - Auto-Generated Skill System
//!
//! This crate provides the skill system for Cratos, enabling:
//! - Pattern detection from usage history
//! - Auto-generation of skills from detected patterns
//! - Keyword/intent-based skill routing
//! - Skill execution with variable interpolation
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  User Input                                                  │
//! └─────────────────────────────────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  SkillRouter                                                 │
//! │  - Keyword matching                                         │
//! │  - Regex pattern matching                                   │
//! │  - Intent classification                                    │
//! └─────────────────────────────────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  SkillExecutor                                               │
//! │  - Variable interpolation                                   │
//! │  - Step execution                                           │
//! │  - Error handling & retries                                 │
//! └─────────────────────────────────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Tool Execution (via cratos-tools)                          │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Pattern Detection & Skill Generation
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  cratos-replay EventStore                                    │
//! │  (Execution history)                                        │
//! └─────────────────────────────────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  PatternAnalyzer                                             │
//! │  - Extract tool sequences                                   │
//! │  - N-gram analysis                                          │
//! │  - Keyword extraction                                       │
//! └─────────────────────────────────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  SkillGenerator                                              │
//! │  - Generate skill from pattern                              │
//! │  - Create steps & triggers                                  │
//! │  - Generate input schema                                    │
//! └─────────────────────────────────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  SkillStore (SQLite)                                         │
//! │  - Persist skills                                           │
//! │  - Track patterns                                           │
//! │  - Record executions                                        │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example Usage
//!
//! ```ignore
//! use cratos_skills::{
//!     SkillStore, SkillRegistry, SkillRouter,
//!     PatternAnalyzer, SkillGenerator,
//! };
//!
//! // Initialize store
//! let store = SkillStore::from_path(&default_db_path()).await?;
//!
//! // Load skills into registry
//! let registry = SkillRegistry::new();
//! let skills = store.list_active_skills().await?;
//! registry.load_all(skills).await?;
//!
//! // Route user input to skill
//! let mut router = SkillRouter::new(registry);
//! if let Some(result) = router.route_best("read file and commit").await {
//!     println!("Matched skill: {} (score: {:.2})", result.skill.name, result.score);
//! }
//!
//! // Detect patterns from history
//! let analyzer = PatternAnalyzer::new();
//! let patterns = analyzer.detect_patterns(&event_store).await?;
//!
//! // Generate skills from patterns
//! let generator = SkillGenerator::new();
//! for pattern in &patterns {
//!     let skill = generator.generate_from_pattern(pattern)?;
//!     store.save_skill(&skill).await?;
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod analyzer;
pub mod error;
pub mod executor;
pub mod generator;
pub mod registry;
pub mod router;
pub mod skill;
pub mod store;

// Re-export main types
pub use analyzer::{AnalyzerConfig, DetectedPattern, PatternAnalyzer, PatternStatus};
pub use error::{Error, Result};
pub use executor::{ExecutorConfig, SkillExecutionResult, SkillExecutor, StepResult, ToolExecutor};
pub use generator::{GeneratorConfig, SkillGenerator};
pub use registry::SkillRegistry;
pub use router::{MatchReason, RouterConfig, RoutingResult, SkillRouter};
pub use skill::{
    ErrorAction, Skill, SkillCategory, SkillMetadata, SkillOrigin, SkillStatus, SkillStep,
    SkillTrigger,
};
pub use store::SkillStore;

/// Get the default skill database path (uses cratos-replay's data directory)
pub fn default_skill_db_path() -> std::path::PathBuf {
    cratos_replay::default_data_dir().join("skills.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_skill_db_path() {
        let path = default_skill_db_path();
        assert!(path.to_string_lossy().contains("skills.db"));
    }
}
