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
mod tests {
    use super::*;

    #[test]
    fn test_working_memory() {
        let mut wm = WorkingMemory::new();

        wm.set("foo", serde_json::json!("bar"));
        assert_eq!(wm.get("foo"), Some(&serde_json::json!("bar")));

        wm.record_tool_execution(
            "test_tool",
            serde_json::json!({}),
            Some(serde_json::json!({"result": "ok"})),
            true,
            None,
        );
        assert_eq!(wm.tool_history.len(), 1);
        assert!(wm.last_tool_execution().unwrap().success);
    }

    #[test]
    fn test_working_memory_with_execution_id() {
        let id = Uuid::new_v4();
        let wm = WorkingMemory::with_execution_id(id);
        assert_eq!(wm.execution_id, Some(id));
    }

    #[test]
    fn test_clear() {
        let mut wm = WorkingMemory::new();
        wm.set("key", serde_json::json!("value"));
        wm.current_step = 5;
        wm.total_steps = 10;

        wm.clear();
        assert!(wm.variables.is_empty());
        assert_eq!(wm.current_step, 0);
        assert_eq!(wm.total_steps, 0);
    }
}
