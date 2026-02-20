//! Persona-Skill Binding Types
//!
//! This module defines the data structures for binding skills to personas,
//! enabling persona-specific skill ownership, metrics tracking, and auto-assignment.
//!
//! # Overview
//!
//! Each persona can own skills with three ownership types:
//! - **Default**: Declared in the persona's TOML `[skills.default]` section
//! - **Claimed**: Manually assigned via CLI or API
//! - **AutoAssigned**: Automatically assigned based on usage patterns
//!
//! # Auto-Assignment
//!
//! Skills are automatically assigned to a persona when:
//! 1. `consecutive_successes >= 5`
//! 2. `success_rate >= 0.8`
//! 3. `usage_count >= 3`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How a persona acquired ownership of a skill
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipType {
    /// Declared in persona's TOML `[skills.default]` section
    Default,
    /// Manually assigned via CLI or API
    Claimed,
    /// Automatically assigned based on usage patterns
    AutoAssigned,
}

impl OwnershipType {
    /// Returns the string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Claimed => "claimed",
            Self::AutoAssigned => "auto_assigned",
        }
    }
}

impl std::fmt::Display for OwnershipType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for OwnershipType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(Self::Default),
            "claimed" => Ok(Self::Claimed),
            "auto_assigned" => Ok(Self::AutoAssigned),
            _ => Err(format!("unknown ownership type: {s}")),
        }
    }
}

impl Default for OwnershipType {
    fn default() -> Self {
        Self::Claimed
    }
}

/// A binding between a persona and a skill with tracking metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaSkillBinding {
    /// Unique identifier for this binding
    pub id: Uuid,
    /// Persona name (e.g., "sindri", "athena")
    pub persona_name: String,
    /// Skill ID
    pub skill_id: Uuid,
    /// Skill name (denormalized for convenience)
    pub skill_name: String,
    /// How the persona acquired this skill
    pub ownership_type: OwnershipType,

    // === Metrics ===
    /// Total number of times this persona used this skill
    pub usage_count: u64,
    /// Number of successful executions
    pub success_count: u64,
    /// Number of failed executions
    pub failure_count: u64,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Average execution duration in milliseconds
    pub avg_duration_ms: Option<u64>,
    /// Last time this skill was used by this persona
    pub last_used_at: Option<DateTime<Utc>>,

    // === Auto-assignment tracking ===
    /// Consecutive successful executions (resets on failure)
    pub consecutive_successes: u32,
    /// When the skill was auto-assigned (if applicable)
    pub auto_assigned_at: Option<DateTime<Utc>>,

    // === Timestamps ===
    /// When this binding was created
    pub created_at: DateTime<Utc>,
    /// When this binding was last updated
    pub updated_at: DateTime<Utc>,
}

impl PersonaSkillBinding {
    /// Create a new binding with default ownership type
    pub fn new(
        persona_name: impl Into<String>,
        skill_id: Uuid,
        skill_name: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            persona_name: persona_name.into(),
            skill_id,
            skill_name: skill_name.into(),
            ownership_type: OwnershipType::Claimed,
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            success_rate: 1.0,
            avg_duration_ms: None,
            last_used_at: None,
            consecutive_successes: 0,
            auto_assigned_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a default binding (from TOML)
    pub fn default_binding(
        persona_name: impl Into<String>,
        skill_id: Uuid,
        skill_name: impl Into<String>,
    ) -> Self {
        let mut binding = Self::new(persona_name, skill_id, skill_name);
        binding.ownership_type = OwnershipType::Default;
        binding
    }

    /// Record a successful execution
    pub fn record_success(&mut self, duration_ms: u64) {
        self.usage_count += 1;
        self.success_count += 1;
        self.consecutive_successes += 1;
        self.last_used_at = Some(Utc::now());
        self.updated_at = Utc::now();

        // Update success rate
        self.success_rate = self.success_count as f64 / self.usage_count as f64;

        // Update average duration
        if let Some(avg) = self.avg_duration_ms {
            self.avg_duration_ms =
                Some((avg * (self.usage_count - 1) + duration_ms) / self.usage_count);
        } else {
            self.avg_duration_ms = Some(duration_ms);
        }
    }

    /// Record a failed execution
    pub fn record_failure(&mut self) {
        self.usage_count += 1;
        self.failure_count += 1;
        self.consecutive_successes = 0; // Reset streak
        self.last_used_at = Some(Utc::now());
        self.updated_at = Utc::now();

        // Update success rate
        self.success_rate = self.success_count as f64 / self.usage_count as f64;
    }

    /// Mark as auto-assigned
    pub fn mark_auto_assigned(&mut self) {
        self.ownership_type = OwnershipType::AutoAssigned;
        self.auto_assigned_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Check if this binding qualifies for auto-assignment
    pub fn qualifies_for_auto_assignment(&self, config: &AutoAssignmentConfig) -> bool {
        if !config.enabled {
            return false;
        }
        // Already auto-assigned or claimed means no need to check
        if matches!(
            self.ownership_type,
            OwnershipType::AutoAssigned | OwnershipType::Default
        ) {
            return false;
        }
        self.consecutive_successes >= config.min_consecutive_successes
            && self.success_rate >= config.min_success_rate
            && self.usage_count >= config.min_usage_count
    }

    /// Check if this persona is proficient at this skill (success_rate >= threshold)
    pub fn is_proficient(&self, threshold: f64) -> bool {
        self.usage_count >= 3 && self.success_rate >= threshold
    }
}

/// Configuration for automatic skill assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoAssignmentConfig {
    /// Whether auto-assignment is enabled
    pub enabled: bool,
    /// Minimum consecutive successes required for auto-assignment
    pub min_consecutive_successes: u32,
    /// Minimum success rate required for auto-assignment (0.0 - 1.0)
    pub min_success_rate: f64,
    /// Minimum usage count required for auto-assignment
    pub min_usage_count: u64,
    /// Bonus score to add when routing to persona's proficient skills
    pub persona_skill_bonus: f32,
    /// Success rate threshold for proficiency bonus
    pub proficiency_threshold: f64,
}

impl Default for AutoAssignmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_consecutive_successes: 5,
            min_success_rate: 0.8,
            min_usage_count: 3,
            persona_skill_bonus: 0.2,
            proficiency_threshold: 0.7,
        }
    }
}

impl AutoAssignmentConfig {
    /// Create a new config with auto-assignment disabled
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a stricter configuration
    #[must_use]
    pub fn strict() -> Self {
        Self {
            enabled: true,
            min_consecutive_successes: 10,
            min_success_rate: 0.9,
            min_usage_count: 5,
            persona_skill_bonus: 0.3,
            proficiency_threshold: 0.8,
        }
    }
}

/// Record of a single skill execution by a persona
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaSkillExecution {
    /// Unique identifier
    pub id: Uuid,
    /// Persona name
    pub persona_name: String,
    /// Skill ID
    pub skill_id: Uuid,
    /// Related execution ID (from orchestrator)
    pub execution_id: Option<Uuid>,
    /// Whether the execution succeeded
    pub success: bool,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Error message (if failed)
    pub error_message: Option<String>,
    /// When the execution started
    pub started_at: DateTime<Utc>,
}

impl PersonaSkillExecution {
    /// Create a new execution record
    pub fn new(persona_name: impl Into<String>, skill_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            persona_name: persona_name.into(),
            skill_id,
            execution_id: None,
            success: true,
            duration_ms: None,
            error_message: None,
            started_at: Utc::now(),
        }
    }

    /// Set execution ID
    #[must_use]
    pub fn with_execution_id(mut self, execution_id: Uuid) -> Self {
        self.execution_id = Some(execution_id);
        self
    }

    /// Mark as failed with error message
    #[must_use]
    pub fn failed(mut self, error: impl Into<String>) -> Self {
        self.success = false;
        self.error_message = Some(error.into());
        self
    }

    /// Set duration
    #[must_use]
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ownership_type_serialization() {
        let owned = OwnershipType::AutoAssigned;
        let json = serde_json::to_string(&owned).unwrap();
        assert_eq!(json, r#""auto_assigned""#);

        let parsed: OwnershipType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, OwnershipType::AutoAssigned);
    }

    #[test]
    fn test_ownership_type_from_str() {
        assert_eq!(
            "default".parse::<OwnershipType>().unwrap(),
            OwnershipType::Default
        );
        assert_eq!(
            "claimed".parse::<OwnershipType>().unwrap(),
            OwnershipType::Claimed
        );
        assert_eq!(
            "auto_assigned".parse::<OwnershipType>().unwrap(),
            OwnershipType::AutoAssigned
        );
        assert!("invalid".parse::<OwnershipType>().is_err());
    }

    #[test]
    fn test_persona_skill_binding_new() {
        let skill_id = Uuid::new_v4();
        let binding = PersonaSkillBinding::new("sindri", skill_id, "api_builder");

        assert_eq!(binding.persona_name, "sindri");
        assert_eq!(binding.skill_id, skill_id);
        assert_eq!(binding.skill_name, "api_builder");
        assert_eq!(binding.ownership_type, OwnershipType::Claimed);
        assert_eq!(binding.usage_count, 0);
        assert_eq!(binding.success_rate, 1.0);
        assert_eq!(binding.consecutive_successes, 0);
    }

    #[test]
    fn test_default_binding() {
        let skill_id = Uuid::new_v4();
        let binding = PersonaSkillBinding::default_binding("sindri", skill_id, "rust_dev");

        assert_eq!(binding.ownership_type, OwnershipType::Default);
    }

    #[test]
    fn test_record_success() {
        let skill_id = Uuid::new_v4();
        let mut binding = PersonaSkillBinding::new("sindri", skill_id, "test");

        binding.record_success(100);
        assert_eq!(binding.usage_count, 1);
        assert_eq!(binding.success_count, 1);
        assert_eq!(binding.consecutive_successes, 1);
        assert_eq!(binding.success_rate, 1.0);
        assert_eq!(binding.avg_duration_ms, Some(100));

        binding.record_success(200);
        assert_eq!(binding.usage_count, 2);
        assert_eq!(binding.consecutive_successes, 2);
        assert_eq!(binding.avg_duration_ms, Some(150)); // (100 + 200) / 2
    }

    #[test]
    fn test_record_failure() {
        let skill_id = Uuid::new_v4();
        let mut binding = PersonaSkillBinding::new("sindri", skill_id, "test");

        binding.record_success(100);
        binding.record_success(100);
        binding.record_failure();

        assert_eq!(binding.usage_count, 3);
        assert_eq!(binding.success_count, 2);
        assert_eq!(binding.failure_count, 1);
        assert_eq!(binding.consecutive_successes, 0); // Reset on failure
        assert!((binding.success_rate - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_auto_assignment_qualification() {
        let skill_id = Uuid::new_v4();
        let mut binding = PersonaSkillBinding::new("sindri", skill_id, "test");
        let config = AutoAssignmentConfig::default();

        // Initially not qualified
        assert!(!binding.qualifies_for_auto_assignment(&config));

        // Record 5 consecutive successes
        for _ in 0..5 {
            binding.record_success(100);
        }

        // Now should qualify
        assert!(binding.qualifies_for_auto_assignment(&config));

        // After marking as auto-assigned, should not qualify again
        binding.mark_auto_assigned();
        assert!(!binding.qualifies_for_auto_assignment(&config));
    }

    #[test]
    fn test_auto_assignment_config_disabled() {
        let skill_id = Uuid::new_v4();
        let mut binding = PersonaSkillBinding::new("sindri", skill_id, "test");
        let config = AutoAssignmentConfig::disabled();

        for _ in 0..10 {
            binding.record_success(100);
        }

        // Even with many successes, disabled config should not allow auto-assignment
        assert!(!binding.qualifies_for_auto_assignment(&config));
    }

    #[test]
    fn test_is_proficient() {
        let skill_id = Uuid::new_v4();
        let mut binding = PersonaSkillBinding::new("sindri", skill_id, "test");

        // Not proficient with less than 3 uses
        binding.record_success(100);
        binding.record_success(100);
        assert!(!binding.is_proficient(0.7));

        // Proficient after 3 uses with good success rate
        binding.record_success(100);
        assert!(binding.is_proficient(0.7));

        // Not proficient if success rate drops below threshold
        binding.record_failure();
        binding.record_failure();
        // Success rate = 3/5 = 0.6 < 0.7
        assert!(!binding.is_proficient(0.7));
    }

    #[test]
    fn test_persona_skill_execution() {
        let skill_id = Uuid::new_v4();
        let exec_id = Uuid::new_v4();

        let exec = PersonaSkillExecution::new("sindri", skill_id)
            .with_execution_id(exec_id)
            .with_duration(150);

        assert_eq!(exec.persona_name, "sindri");
        assert_eq!(exec.skill_id, skill_id);
        assert_eq!(exec.execution_id, Some(exec_id));
        assert!(exec.success);
        assert_eq!(exec.duration_ms, Some(150));

        let failed = PersonaSkillExecution::new("sindri", skill_id).failed("timeout");
        assert!(!failed.success);
        assert_eq!(failed.error_message, Some("timeout".to_string()));
    }

    #[test]
    fn test_auto_assignment_config_strict() {
        let config = AutoAssignmentConfig::strict();
        assert!(config.enabled);
        assert_eq!(config.min_consecutive_successes, 10);
        assert_eq!(config.min_success_rate, 0.9);
        assert_eq!(config.min_usage_count, 5);
    }
}
