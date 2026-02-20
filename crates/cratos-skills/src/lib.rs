//! # Cratos Skills - Auto-Generated Skill System
//!
//! This crate provides the core skill system for Cratos, enabling automatic
//! learning and generation of reusable workflows from user behavior patterns.
//!
//! ## Key Features
//!
//! - **Pattern Detection**: Automatically detects recurring tool usage patterns (3+ occurrences)
//! - **Skill Generation**: Converts detected patterns into executable skills with 90%+ success rate
//! - **Smart Routing**: Routes user requests to appropriate skills via keyword, regex, or semantic matching
//! - **Variable Interpolation**: Supports `{{variable}}` syntax for dynamic skill parameters
//! - **Execution Tracking**: Records all skill executions for continuous improvement
//!
//! ## Core Components
//!
//! | Component | Description |
//! |-----------|-------------|
//! | [`PatternAnalyzer`] | Detects usage patterns from execution history |
//! | [`SkillGenerator`] | Creates skills from detected patterns |
//! | [`SkillStore`] | SQLite-based persistent storage |
//! | [`SkillRegistry`] | In-memory skill registry with keyword indexing |
//! | [`SkillRouter`] | Routes requests to matching skills |
//! | [`SkillExecutor`] | Executes skill workflows with variable interpolation |
//!
//! ## Architecture
//!
//! ```text
//! User Input
//!     │
//!     ▼
//! ┌────────────────────────────────────────────────────────────┐
//! │  SkillRouter                                                │
//! │  • Keyword matching      • Regex pattern matching          │
//! │  • Intent classification • Priority-based selection        │
//! └────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! ┌────────────────────────────────────────────────────────────┐
//! │  SkillExecutor                                              │
//! │  • Variable interpolation ({{var}} → value)                │
//! │  • Step-by-step execution with error handling              │
//! │  • Dry-run mode for testing                                │
//! └────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! Tool Execution (cratos-tools)
//! ```
//!
//! ## Pattern Detection & Skill Generation Flow
//!
//! ```text
//! cratos-replay EventStore (execution history)
//!     │
//!     ▼
//! ┌────────────────────────────────────────────────────────────┐
//! │  PatternAnalyzer                                            │
//! │  • Extract tool sequences from executions                  │
//! │  • N-gram analysis (2-5 tool combinations)                 │
//! │  • Keyword extraction (stopword removal)                   │
//! │  • Confidence score calculation                            │
//! └────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! ┌────────────────────────────────────────────────────────────┐
//! │  SkillGenerator                                             │
//! │  • Pattern → Skill conversion                              │
//! │  • Trigger keywords & regex patterns                       │
//! │  • Input schema generation (JSON Schema)                   │
//! └────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! ┌────────────────────────────────────────────────────────────┐
//! │  SkillStore (SQLite: ~/.cratos/skills.db)                   │
//! │  • Persist skills & patterns                               │
//! │  • Track execution history                                 │
//! │  • Manage skill lifecycle (Draft → Active → Disabled)      │
//! └────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```ignore
//! use cratos_skills::{
//!     SkillStore, SkillRegistry, SkillRouter,
//!     PatternAnalyzer, SkillGenerator, default_skill_db_path,
//! };
//!
//! // Initialize persistent store
//! let store = SkillStore::from_path(&default_skill_db_path()).await?;
//!
//! // Load active skills into registry
//! let registry = SkillRegistry::new();
//! let skills = store.list_active_skills().await?;
//! registry.load_all(skills).await?;
//!
//! // Route user input to best matching skill
//! let mut router = SkillRouter::new(registry);
//! if let Some(result) = router.route_best("read file and commit").await {
//!     println!("Matched: {} (score: {:.2})", result.skill.name, result.score);
//! }
//!
//! // Detect patterns from execution history
//! let analyzer = PatternAnalyzer::new();
//! let patterns = analyzer.detect_patterns(&event_store).await?;
//!
//! // Generate skills from high-confidence patterns
//! let generator = SkillGenerator::new();
//! for pattern in patterns.iter().filter(|p| p.confidence_score >= 0.7) {
//!     let skill = generator.generate_from_pattern(pattern)?;
//!     store.save_skill(&skill).await?;
//! }
//! ```
//!
//! ## Security
//!
//! The skill system includes built-in security measures:
//!
//! - **Input validation**: Maximum input length limits (DoS prevention)
//! - **Regex safety**: Pattern length limits (ReDoS prevention)
//! - **Execution limits**: Maximum steps per skill, variable size limits
//! - **Timeout handling**: Per-step timeout configuration
//!
//! ## Feature Flags
//!
//! - `default`: Basic skill system
//! - `semantic`: Enable semantic routing with vector embeddings (requires `cratos-search`)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod analyzer;
pub mod ecosystem;
pub mod error;
pub mod executor;
pub mod generator;
/// Persona skill bindings and ownership tracking
pub mod persona;
pub mod protocol;
pub mod registry;
#[cfg(feature = "remote")]
pub mod remote;
/// Skill routing logic
pub mod routing;
pub mod skill;
pub mod store;

// Re-export main types
pub use analyzer::{AnalyzerConfig, DetectedPattern, PatternAnalyzer, PatternStatus};
pub use error::{Error, Result};
pub use executor::{ExecutorConfig, SkillExecutionResult, SkillExecutor, StepResult, ToolExecutor};
pub use generator::{GeneratorConfig, SkillGenerator};
pub use registry::SkillRegistry;
pub use routing::{MatchReason, RouterConfig, RoutingResult, SkillRouter};
pub use skill::{
    ErrorAction, Skill, SkillCategory, SkillMetadata, SkillOrigin, SkillStatus, SkillStep,
    SkillTrigger,
};
pub use store::SkillStore;

// Re-export persona binding types
pub use persona::{
    AutoAssignmentConfig, OwnershipType, PersonaSkillBinding, PersonaSkillExecution,
    PersonaSkillStore,
};

// Re-export ecosystem types for skill sharing
pub use ecosystem::{
    ExportFormat, ExportInfo, ImportResult, PortableSkill, PortableSkillDef, PortableStep,
    PortableTrigger, SkillBundle, SkillEcosystem,
};

// Re-export unified protocol types
pub use protocol::{
    McpToolWrapper, SkillWrapper, ToolSource, UnifiedError, UnifiedOutput, UnifiedRegistry,
    UnifiedResult, UnifiedTool,
};

// Re-export remote registry when feature is enabled
#[cfg(feature = "remote")]
pub use remote::{RegistryEntry, RemoteRegistry};

// Re-export semantic router when feature is enabled
#[cfg(feature = "semantic")]
pub use routing::semantic::{
    create_skill_index, SemanticMatchReason, SemanticRouterConfig, SemanticRoutingResult,
    SemanticSkillRouter, SkillEmbedder,
};

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
