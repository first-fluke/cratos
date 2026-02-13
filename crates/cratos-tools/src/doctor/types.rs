//! Diagnostic types for tool doctor

use super::category::FailureCategory;
use serde::{Deserialize, Serialize};

/// A diagnosis result from the tool doctor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnosis {
    /// Tool that failed
    pub tool_name: String,
    /// Original error message
    pub error_message: String,
    /// Detected failure category
    pub category: FailureCategory,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Probable causes ranked by likelihood
    pub probable_causes: Vec<ProbableCause>,
    /// Resolution checklist
    pub checklist: Vec<ChecklistItem>,
    /// Alternative approaches if this tool can't work
    pub alternatives: Vec<Alternative>,
}

/// A probable cause with likelihood
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbableCause {
    /// Description of the cause
    pub description: String,
    /// Likelihood percentage (0-100)
    pub likelihood: u8,
    /// How to verify this is the cause
    pub verification: String,
}

/// A checklist item for resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    /// Step number
    pub step: u8,
    /// Action to take
    pub action: String,
    /// Command or instruction
    pub instruction: String,
    /// Expected result
    pub expected_result: String,
}

/// An alternative approach
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    /// Description of alternative
    pub description: String,
    /// Tool to use instead (if any)
    pub tool_name: Option<String>,
    /// Trade-offs of this approach
    pub tradeoffs: String,
}
