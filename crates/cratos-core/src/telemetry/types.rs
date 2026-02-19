use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Telemetry event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelemetryEvent {
    /// Application started
    AppStarted {
        /// Cratos version
        version: String,
        /// Platform (darwin, linux, windows)
        platform: String,
    },

    /// Command executed
    CommandExecuted {
        /// Command name (anonymized)
        command: String,
        /// Execution duration in milliseconds
        duration_ms: u64,
        /// Whether the command succeeded
        success: bool,
    },

    /// Channel used
    ChannelUsed {
        /// Channel type (telegram, slack, matrix, etc.)
        channel_type: String,
    },

    /// LLM provider used
    LlmUsed {
        /// Provider name (openai, anthropic, etc.)
        provider: String,
        /// Model tier (fast, standard, premium)
        tier: String,
        /// Token count
        tokens: u32,
    },

    /// Tool executed
    ToolExecuted {
        /// Tool category (file, http, git, etc.)
        category: String,
        /// Execution duration in milliseconds
        duration_ms: u64,
        /// Whether the tool succeeded
        success: bool,
    },

    /// Skill used
    SkillUsed {
        /// Skill origin (builtin, user_defined, auto_generated)
        origin: String,
        /// Whether the skill succeeded
        success: bool,
    },

    /// Feature used
    FeatureUsed {
        /// Feature name
        feature: String,
    },

    /// Error occurred (category only, no details)
    ErrorOccurred {
        /// Error category
        category: String,
    },
}

/// Telemetry event with metadata (internal use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRecord {
    pub timestamp: DateTime<Utc>,
    pub anonymous_id: String,
    pub event: TelemetryEvent,
}
