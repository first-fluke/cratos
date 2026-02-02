//! Registry - Tool registration and discovery
//!
//! This module provides a registry for managing tools that can be used
//! by the AI assistant. Tools are registered with metadata and can be
//! queried by name, risk level, or category.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// Risk level of a tool
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// Low risk - read-only operations
    Low,
    /// Medium risk - write operations with limited scope
    Medium,
    /// High risk - system modifications, command execution
    High,
}

impl RiskLevel {
    /// Returns the string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// Check if approval is required for this risk level
    #[must_use]
    pub fn requires_approval(&self) -> bool {
        matches!(self, Self::High)
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Tool category for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolCategory {
    /// File operations
    File,
    /// HTTP/network operations
    Http,
    /// Command execution
    Exec,
    /// Git operations
    Git,
    /// Search operations
    Search,
    /// Utility operations
    Utility,
}

impl ToolCategory {
    /// Returns the string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Http => "http",
            Self::Exec => "exec",
            Self::Git => "git",
            Self::Search => "search",
            Self::Utility => "utility",
        }
    }
}

/// Tool metadata and schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// JSON schema for parameters
    pub parameters: serde_json::Value,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Tool category
    pub category: ToolCategory,
    /// Whether the tool is enabled
    pub enabled: bool,
    /// Required permissions/capabilities
    #[serde(default)]
    pub required_capabilities: Vec<String>,
}

impl ToolDefinition {
    /// Create a new tool definition
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            risk_level: RiskLevel::Low,
            category: ToolCategory::Utility,
            enabled: true,
            required_capabilities: Vec::new(),
        }
    }

    /// Set the parameters schema
    #[must_use]
    pub fn with_parameters(mut self, parameters: serde_json::Value) -> Self {
        self.parameters = parameters;
        self
    }

    /// Set the risk level
    #[must_use]
    pub fn with_risk_level(mut self, risk_level: RiskLevel) -> Self {
        self.risk_level = risk_level;
        self
    }

    /// Set the category
    #[must_use]
    pub fn with_category(mut self, category: ToolCategory) -> Self {
        self.category = category;
        self
    }

    /// Set enabled status
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Add a required capability
    #[must_use]
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.required_capabilities.push(capability.into());
        self
    }
}

/// Result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Output data
    pub output: serde_json::Value,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

impl ToolResult {
    /// Create a successful result
    #[must_use]
    pub fn success(output: serde_json::Value, duration_ms: u64) -> Self {
        Self {
            success: true,
            output,
            error: None,
            duration_ms,
        }
    }

    /// Create a failed result
    #[must_use]
    pub fn failure(error: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            success: false,
            output: serde_json::Value::Null,
            error: Some(error.into()),
            duration_ms,
        }
    }
}

/// Trait for tool implementations
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool definition
    fn definition(&self) -> &ToolDefinition;

    /// Execute the tool with given input
    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult>;

    /// Validate input before execution
    fn validate_input(&self, input: &serde_json::Value) -> Result<()> {
        // Default implementation: basic type checking
        if !input.is_object() {
            return Err(Error::InvalidInput("Input must be an object".to_string()));
        }
        Ok(())
    }
}

/// Registry for managing tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    definitions: HashMap<String, ToolDefinition>,
    /// Allowlist for exec command (if empty, all commands allowed)
    exec_allowlist: Vec<String>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            definitions: HashMap::new(),
            exec_allowlist: Vec::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let def = tool.definition();
        let name = def.name.clone();
        debug!(tool = %name, "Registering tool");
        self.definitions.insert(name.clone(), def.clone());
        self.tools.insert(name, tool);
    }

    /// Get a tool by name
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Get a tool definition by name
    #[must_use]
    pub fn get_definition(&self, name: &str) -> Option<&ToolDefinition> {
        self.definitions.get(name)
    }

    /// Check if a tool exists
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// List all tool names
    #[must_use]
    pub fn list_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// List all tool definitions
    #[must_use]
    pub fn list_definitions(&self) -> Vec<&ToolDefinition> {
        self.definitions.values().collect()
    }

    /// List enabled tool definitions
    #[must_use]
    pub fn list_enabled(&self) -> Vec<&ToolDefinition> {
        self.definitions.values().filter(|d| d.enabled).collect()
    }

    /// List tools by category
    #[must_use]
    pub fn list_by_category(&self, category: ToolCategory) -> Vec<&ToolDefinition> {
        self.definitions
            .values()
            .filter(|d| d.category == category)
            .collect()
    }

    /// List tools by risk level
    #[must_use]
    pub fn list_by_risk(&self, risk_level: RiskLevel) -> Vec<&ToolDefinition> {
        self.definitions
            .values()
            .filter(|d| d.risk_level == risk_level)
            .collect()
    }

    /// Set the exec command allowlist
    pub fn set_exec_allowlist(&mut self, allowlist: Vec<String>) {
        self.exec_allowlist = allowlist;
    }

    /// Get the exec command allowlist
    #[must_use]
    pub fn exec_allowlist(&self) -> &[String] {
        &self.exec_allowlist
    }

    /// Check if a command is allowed
    #[must_use]
    pub fn is_command_allowed(&self, command: &str) -> bool {
        if self.exec_allowlist.is_empty() {
            return true;
        }

        // Extract base command (first word)
        let base_command = command.split_whitespace().next().unwrap_or("");

        self.exec_allowlist
            .iter()
            .any(|allowed| base_command == allowed || command.starts_with(allowed))
    }

    /// Enable a tool
    pub fn enable(&mut self, name: &str) -> bool {
        if let Some(def) = self.definitions.get_mut(name) {
            def.enabled = true;
            true
        } else {
            false
        }
    }

    /// Disable a tool
    pub fn disable(&mut self, name: &str) -> bool {
        if let Some(def) = self.definitions.get_mut(name) {
            def.enabled = false;
            true
        } else {
            false
        }
    }

    /// Get tool count
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Convert definitions to LLM tool format
    #[must_use]
    pub fn to_llm_tools(&self) -> Vec<cratos_llm::ToolDefinition> {
        self.list_enabled()
            .into_iter()
            .map(|def| {
                cratos_llm::ToolDefinition::new(&def.name, &def.description, def.parameters.clone())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level() {
        assert_eq!(RiskLevel::Low.as_str(), "low");
        assert!(!RiskLevel::Low.requires_approval());
        assert!(!RiskLevel::Medium.requires_approval());
        assert!(RiskLevel::High.requires_approval());
    }

    #[test]
    fn test_tool_definition_builder() {
        let def = ToolDefinition::new("test_tool", "A test tool")
            .with_risk_level(RiskLevel::Medium)
            .with_category(ToolCategory::File)
            .with_capability("read");

        assert_eq!(def.name, "test_tool");
        assert_eq!(def.risk_level, RiskLevel::Medium);
        assert_eq!(def.category, ToolCategory::File);
        assert!(def.required_capabilities.contains(&"read".to_string()));
    }

    #[test]
    fn test_tool_result() {
        let success = ToolResult::success(serde_json::json!({"data": "test"}), 100);
        assert!(success.success);
        assert!(success.error.is_none());

        let failure = ToolResult::failure("test error", 50);
        assert!(!failure.success);
        assert_eq!(failure.error, Some("test error".to_string()));
    }

    #[test]
    fn test_registry() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_exec_allowlist() {
        let mut registry = ToolRegistry::new();

        // Empty allowlist allows everything
        assert!(registry.is_command_allowed("rm -rf /"));

        // With allowlist, only specified commands allowed
        registry.set_exec_allowlist(vec!["ls".to_string(), "cat".to_string(), "git".to_string()]);
        assert!(registry.is_command_allowed("ls -la"));
        assert!(registry.is_command_allowed("git status"));
        assert!(!registry.is_command_allowed("rm -rf /"));
    }
}
