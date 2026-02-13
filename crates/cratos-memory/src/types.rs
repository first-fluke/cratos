//! Core data types for the graph memory system.
//!
//! The graph connects **turns** (conversation messages) to **entities**
//! (files, functions, crates, tools, errors, concepts) extracted from them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single conversation turn (one user or assistant message).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    /// Unique turn ID (UUID)
    pub id: String,
    /// Session this turn belongs to
    pub session_id: String,
    /// Who sent the message
    pub role: TurnRole,
    /// Full message content
    pub content: String,
    /// Shortened text used for embedding (first ~200 chars + metadata)
    pub summary: String,
    /// Position within the session (0-based)
    pub turn_index: u32,
    /// Approximate token count
    pub token_count: u32,
    /// When this turn was recorded
    pub created_at: DateTime<Utc>,
}

/// Role of a turn (subset of LLM MessageRole, excluding Tool/System).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnRole {
    /// User message
    User,
    /// Assistant response (may include tool call metadata)
    Assistant,
}

impl std::fmt::Display for TurnRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
        }
    }
}

/// An entity extracted from conversation turns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Unique entity ID (UUID)
    pub id: String,
    /// Normalized name (e.g. `"orchestrator.rs"`, `"cratos-core"`)
    pub name: String,
    /// What kind of thing this entity represents
    pub kind: EntityKind,
    /// When this entity was first seen
    pub first_seen: DateTime<Utc>,
    /// How many turns mention this entity
    pub mention_count: u32,
}

/// Classification of an extracted entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityKind {
    /// Source file (`.rs`, `.toml`, `.json`, …)
    File,
    /// Function or method name
    Function,
    /// Rust crate (e.g. `cratos-core`)
    Crate,
    /// Tool name from the registry
    Tool,
    /// Error type or message pattern
    Error,
    /// Technical concept or keyword
    Concept,
    /// Configuration key
    Config,
}

impl std::fmt::Display for EntityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => write!(f, "file"),
            Self::Function => write!(f, "function"),
            Self::Crate => write!(f, "crate"),
            Self::Tool => write!(f, "tool"),
            Self::Error => write!(f, "error"),
            Self::Concept => write!(f, "concept"),
            Self::Config => write!(f, "config"),
        }
    }
}

impl EntityKind {
    /// Parse from string.
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "file" => Self::File,
            "function" => Self::Function,
            "crate" => Self::Crate,
            "tool" => Self::Tool,
            "error" => Self::Error,
            "concept" => Self::Concept,
            "config" => Self::Config,
            _ => Self::Concept, // fallback
        }
    }
}

/// An edge connecting a turn to an entity it mentions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnEntityEdge {
    /// Turn that mentions this entity
    pub turn_id: String,
    /// Entity mentioned in the turn
    pub entity_id: String,
    /// How relevant the entity is to this turn (0.0–1.0)
    pub relevance: f32,
}

/// A turn retrieved by the graph search, with scoring metadata.
#[derive(Debug, Clone)]
pub struct RetrievedTurn {
    /// The turn itself
    pub turn: Turn,
    /// Composite retrieval score
    pub score: f32,
    /// Entity names that contributed to this match
    pub matched_entities: Vec<String>,
}

/// An explicitly saved memory (user-requested knowledge).
///
/// Unlike auto-indexed turns, these represent knowledge the user
/// explicitly asked to remember, with a descriptive name and tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplicitMemory {
    /// Unique memory ID (UUID)
    pub id: String,
    /// Human-readable name (e.g. "stealth-native-logic-v1")
    pub name: String,
    /// Full memory content
    pub content: String,
    /// Category (general, knowledge, blueprint, strategy, pattern, error_fix)
    pub category: String,
    /// Searchable tags
    pub tags: Vec<String>,
    /// When this memory was created
    pub created_at: DateTime<Utc>,
    /// When this memory was last updated
    pub updated_at: DateTime<Utc>,
    /// Number of times this memory was recalled
    pub access_count: u32,
}

/// An extracted entity with its relevance score (before persistence).
#[derive(Debug, Clone)]
pub struct ExtractedEntity {
    /// Entity name (normalized)
    pub name: String,
    /// Entity kind
    pub kind: EntityKind,
    /// Relevance to the source turn (0.0–1.0)
    pub relevance: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_role_display() {
        assert_eq!(TurnRole::User.to_string(), "user");
        assert_eq!(TurnRole::Assistant.to_string(), "assistant");
    }

    #[test]
    fn test_entity_kind_roundtrip() {
        for kind in [
            EntityKind::File,
            EntityKind::Function,
            EntityKind::Crate,
            EntityKind::Tool,
            EntityKind::Error,
            EntityKind::Concept,
            EntityKind::Config,
        ] {
            let s = kind.to_string();
            assert_eq!(EntityKind::from_str_lossy(&s), kind);
        }
    }

    #[test]
    fn test_entity_kind_unknown_fallback() {
        assert_eq!(EntityKind::from_str_lossy("unknown"), EntityKind::Concept);
    }

    #[test]
    fn test_turn_serialization() {
        let turn = Turn {
            id: "abc".into(),
            session_id: "sess1".into(),
            role: TurnRole::User,
            content: "hello".into(),
            summary: "hello".into(),
            turn_index: 0,
            token_count: 5,
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&turn).unwrap();
        let back: Turn = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "abc");
        assert_eq!(back.role, TurnRole::User);
    }
}
