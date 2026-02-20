//! Law Enforcement Engine
//!
//! Validates persona responses against Laws and applies enforcement actions.

use crate::chronicles::ChronicleStore;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Types of law violations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LawViolation {
    /// Missing response format per Laws Article 6
    MissingResponseFormat {
        /// Persona that violated
        persona: String,
    },
    /// Missing commit hash per Laws Article 10
    MissingCommitHash {
        /// Persona that violated
        persona: String,
    },
}

impl LawViolation {
    /// Get the law article reference for this violation
    #[must_use]
    pub fn article_ref(&self) -> &'static str {
        match self {
            Self::MissingResponseFormat { .. } => "6",
            Self::MissingCommitHash { .. } => "10",
        }
    }

    /// Get a human-readable description
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::MissingResponseFormat { persona } => {
                format!(
                    "{persona}: Response missing required format [Role] Name LvN : Message (Art.6)"
                )
            }
            Self::MissingCommitHash { persona } => {
                format!("{persona}: Task completion without commit hash (Art.10)")
            }
        }
    }
}

/// Actions taken by the enforcer
#[derive(Debug, Clone)]
pub enum EnforcementAction {
    /// Warning logged to chronicle
    Warning {
        /// Persona name
        persona: String,
        /// Warning message
        message: String,
    },
    /// Judgment added to chronicle
    JudgmentAdded {
        /// Persona name
        persona: String,
        /// Judgment comment
        comment: String,
        /// Score
        score: f32,
    },
    /// Silence punishment applied (Laws Article 8)
    SilenceApplied {
        /// Persona name
        persona: String,
    },
}

/// Configuration for the law enforcer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforcerConfig {
    /// Whether enforcement is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Whether to automatically apply silence punishment
    #[serde(default)]
    pub auto_silence: bool,
    /// Whether to automatically add judgment entries
    #[serde(default = "default_true")]
    pub auto_judgment: bool,
}

fn default_true() -> bool {
    true
}

impl Default for EnforcerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_silence: false,
            auto_judgment: true,
        }
    }
}

/// Penalty score for missing response format (Article 6) — minor violation
const FORMAT_VIOLATION_SCORE: f32 = 2.5;
/// Penalty score for missing commit hash (Article 10) — moderate violation
const COMMIT_VIOLATION_SCORE: f32 = 2.0;

/// Law Enforcer - validates and enforces compliance
pub struct LawEnforcer {
    config: EnforcerConfig,
    chronicle_store: ChronicleStore,
}

impl LawEnforcer {
    /// Create a new enforcer
    #[must_use]
    pub fn new(config: EnforcerConfig, chronicle_store: ChronicleStore) -> Self {
        Self {
            config,
            chronicle_store,
        }
    }

    /// Validate a response against laws
    #[must_use]
    pub fn validate_response(
        &self,
        persona: &str,
        response: &str,
        task_completed: bool,
    ) -> Vec<LawViolation> {
        if !self.config.enabled {
            return Vec::new();
        }

        let mut violations = Vec::new();

        // Check Laws Article 6: Response format [Role] Name LvN : Message
        // Relaxed: also accept responses that reference the persona name
        let has_role_prefix = response.starts_with('[') && response.contains("] ");
        let has_persona_ref = response.to_lowercase().contains(&persona.to_lowercase());
        if !has_role_prefix && !has_persona_ref {
            violations.push(LawViolation::MissingResponseFormat {
                persona: persona.to_string(),
            });
        }

        // Check Laws Article 10: Commit hash on task completion
        if task_completed && !response.contains('[') {
            // Simple heuristic: look for something that looks like a commit hash
            let has_hash = response
                .split_whitespace()
                .any(|w| w.len() >= 7 && w.chars().all(|c| c.is_ascii_hexdigit()));
            if !has_hash {
                violations.push(LawViolation::MissingCommitHash {
                    persona: persona.to_string(),
                });
            }
        }

        violations
    }

    /// Apply enforcement actions for violations
    pub fn enforce(&self, violations: &[LawViolation]) -> Result<Vec<EnforcementAction>> {
        let mut actions = Vec::new();

        for violation in violations {
            let persona = match violation {
                LawViolation::MissingResponseFormat { persona }
                | LawViolation::MissingCommitHash { persona } => persona,
            };

            // Auto-judgment: record the violation
            if self.config.auto_judgment {
                let comment = violation.description();
                let score = match violation {
                    LawViolation::MissingResponseFormat { .. } => FORMAT_VIOLATION_SCORE,
                    LawViolation::MissingCommitHash { .. } => COMMIT_VIOLATION_SCORE,
                };
                if let Ok(Some(mut chronicle)) = self.chronicle_store.load(persona) {
                    chronicle.add_judgment("Cratos", &comment, Some(score));
                    if let Err(e) = self.chronicle_store.save(&chronicle) {
                        warn!(error = %e, persona = persona, "Failed to save judgment");
                    } else {
                        debug!(persona = persona, "Judgment added for violation");
                        actions.push(EnforcementAction::JudgmentAdded {
                            persona: persona.clone(),
                            comment,
                            score,
                        });
                    }
                } else {
                    actions.push(EnforcementAction::Warning {
                        persona: persona.clone(),
                        message: violation.description(),
                    });
                }
            }

            // Auto-silence: apply silence for violations
            if self.config.auto_silence {
                if let Ok(Some(mut chronicle)) = self.chronicle_store.load(persona) {
                    chronicle.apply_silence();
                    if let Err(e) = self.chronicle_store.save(&chronicle) {
                        warn!(error = %e, persona = persona, "Failed to apply silence");
                    } else {
                        debug!(persona = persona, "Silence applied for violation");
                        actions.push(EnforcementAction::SilenceApplied {
                            persona: persona.clone(),
                        });
                    }
                }
            }
        }

        Ok(actions)
    }
}

#[cfg(test)]
mod tests;

