//! Working memory for execution state

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Working memory for a single execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkingMemory {
    /// Execution ID
    pub execution_id: Option<Uuid>,
    /// Variables set during execution
    pub variables: HashMap<String, serde_json::Value>,
    /// Tool execution history for this execution
    pub tool_history: Vec<ToolExecution>,
    /// Current step in the plan
    pub current_step: usize,
    /// Total steps in the plan
    pub total_steps: usize,
}

/// Record of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecution {
    /// Tool name
    pub tool_name: String,
    /// Input provided
    pub input: serde_json::Value,
    /// Output received
    pub output: Option<serde_json::Value>,
    /// Whether execution succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl WorkingMemory {
    /// Create a new working memory
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with an execution ID
    #[must_use]
    pub fn with_execution_id(execution_id: Uuid) -> Self {
        Self {
            execution_id: Some(execution_id),
            ..Self::default()
        }
    }

    /// Set a variable
    pub fn set(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.variables.insert(key.into(), value);
    }

    /// Get a variable
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.variables.get(key)
    }

    /// Remove a variable
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.variables.remove(key)
    }

    /// Record a tool execution
    pub fn record_tool_execution(
        &mut self,
        tool_name: impl Into<String>,
        input: serde_json::Value,
        output: Option<serde_json::Value>,
        success: bool,
        error: Option<String>,
    ) {
        self.tool_history.push(ToolExecution {
            tool_name: tool_name.into(),
            input,
            output,
            success,
            error,
            timestamp: Utc::now(),
        });
    }

    /// Get the last tool execution
    #[must_use]
    pub fn last_tool_execution(&self) -> Option<&ToolExecution> {
        self.tool_history.last()
    }

    /// Get all tool executions
    #[must_use]
    pub fn tool_history(&self) -> &[ToolExecution] {
        &self.tool_history
    }

    /// Clear the working memory
    pub fn clear(&mut self) {
        self.variables.clear();
        self.tool_history.clear();
        self.current_step = 0;
        self.total_steps = 0;
    }
}

#[cfg(test)]
mod tests;

