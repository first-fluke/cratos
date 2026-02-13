//! Routing rules for model selection
//!
//! This module contains the RoutingRules struct for customizing model routing behavior.

use super::types::{ModelTier, TaskType};
use crate::token::TokenBudget;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Model routing configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingRules {
    /// Task-specific provider overrides
    #[serde(default)]
    pub task_providers: HashMap<TaskType, String>,
    /// Task-specific model overrides
    #[serde(default)]
    pub task_models: HashMap<TaskType, String>,
    /// Task-specific token budget overrides
    #[serde(default)]
    pub task_token_budgets: HashMap<TaskType, TokenBudget>,
    /// Whether to prefer local models when available
    #[serde(default)]
    pub prefer_local: bool,
    /// Maximum cost tier allowed
    #[serde(default)]
    pub max_tier: Option<ModelTier>,
}

impl RoutingRules {
    /// Get token budget for a task type, with custom override or default
    #[must_use]
    pub fn get_token_budget(&self, task_type: TaskType) -> TokenBudget {
        self.task_token_budgets
            .get(&task_type)
            .cloned()
            .unwrap_or_else(|| task_type.default_token_budget())
    }
}
