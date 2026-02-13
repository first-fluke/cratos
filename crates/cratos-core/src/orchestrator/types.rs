//! Orchestrator types and traits
//!
//! Contains core type definitions for orchestration:
//! - `SkillMatch` and `SkillRouting` for skill routing
//! - `ExecutionStatus`, `ExecutionResult`, `ExecutionArtifact` for execution results
//! - `ToolCallRecord` for tool call tracking

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Skill routing match result with full details
#[derive(Debug, Clone)]
pub struct SkillMatch {
    /// Skill ID (UUID) for tracking
    pub skill_id: Uuid,
    /// Skill name
    pub skill_name: String,
    /// Skill description
    pub description: String,
    /// Match score (0.0 - 1.0)
    pub score: f32,
}

/// Trait for routing user input to a matching skill
#[async_trait::async_trait]
pub trait SkillRouting: Send + Sync {
    /// Route input to the best matching skill.
    /// Returns a `SkillMatch` with skill_id for persona-skill tracking.
    async fn route_best(&self, input: &str) -> Option<SkillMatch>;
}

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    /// Execution is pending
    Pending,
    /// Execution is in progress
    Running,
    /// Execution completed successfully
    Completed,
    /// Execution partially succeeded (some personas failed in multi-persona mode)
    PartialSuccess,
    /// Execution failed
    Failed,
    /// Execution was cancelled
    Cancelled,
}

/// Result of an orchestrated execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Execution ID
    pub execution_id: Uuid,
    /// Final status
    pub status: ExecutionStatus,
    /// Response text
    pub response: String,
    /// Tool calls made
    pub tool_calls: Vec<ToolCallRecord>,
    /// Artifacts generated execution (e.g. screenshots, files)
    pub artifacts: Vec<ExecutionArtifact>,
    /// Total iterations
    pub iterations: usize,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Model used
    pub model: Option<String>,
}

/// Artifact generated during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionArtifact {
    /// Artifact name/id
    pub name: String,
    /// MIME type
    pub mime_type: String,
    /// Base64 encoded or raw data (represented as string for now)
    pub data: String,
}

/// Record of a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Tool name
    pub tool_name: String,
    /// Input arguments
    pub input: serde_json::Value,
    /// Output result
    pub output: serde_json::Value,
    /// Whether it succeeded
    pub success: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Persona name that executed this tool (for persona-skill metrics)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_name: Option<String>,
}
