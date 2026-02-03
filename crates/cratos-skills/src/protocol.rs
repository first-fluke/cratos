//! Unified Skill/Tool Protocol
//!
//! This module provides a unified interface for both:
//! - Native Cratos skills (YAML-based)
//! - External MCP tools
//!
//! ## Design Philosophy
//!
//! The unified protocol allows Cratos to seamlessly work with both internal skills
//! and external MCP tools through a common interface, enabling:
//! - Mixed skill/tool workflows
//! - Consistent error handling
//! - Unified logging and tracing
//! - Cross-tool parameter passing

use async_trait::async_trait;
use cratos_tools::{McpClient, McpError, McpTool, ToolDefinition};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use crate::{Skill, SkillExecutor, ToolExecutor};

/// Unified tool/skill execution errors
#[derive(Debug, Error)]
pub enum UnifiedError {
    /// Skill execution error
    #[error("Skill error: {0}")]
    Skill(#[from] crate::Error),

    /// MCP error
    #[error("MCP error: {0}")]
    Mcp(#[from] McpError),

    /// Tool not found
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Execution timeout
    #[error("Execution timed out")]
    Timeout,
}

/// Result type for unified operations
pub type UnifiedResult<T> = std::result::Result<T, UnifiedError>;

/// Unified tool interface
///
/// This trait abstracts over both native skills and MCP tools,
/// providing a common interface for tool discovery and execution.
#[async_trait]
pub trait UnifiedTool: Send + Sync {
    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Get the input schema (JSON Schema)
    fn input_schema(&self) -> &Value;

    /// Get the tool source (native or MCP server name)
    fn source(&self) -> ToolSource;

    /// Execute the tool with given input
    async fn execute(&self, input: Value) -> UnifiedResult<UnifiedOutput>;
}

/// Tool source identifier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolSource {
    /// Native Cratos skill
    Native,
    /// MCP server tool
    Mcp {
        /// Server name
        server: String,
    },
    /// Built-in tool
    Builtin,
}

impl std::fmt::Display for ToolSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolSource::Native => write!(f, "native"),
            ToolSource::Mcp { server } => write!(f, "mcp:{}", server),
            ToolSource::Builtin => write!(f, "builtin"),
        }
    }
}

/// Unified output from tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedOutput {
    /// Success/failure status
    pub success: bool,
    /// Output content
    pub content: String,
    /// Structured data (if available)
    #[serde(default)]
    pub data: Option<Value>,
    /// Error message (if failed)
    #[serde(default)]
    pub error: Option<String>,
    /// Execution metadata
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}

impl UnifiedOutput {
    /// Create a successful output
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            data: None,
            error: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a successful output with structured data
    pub fn success_with_data(content: impl Into<String>, data: Value) -> Self {
        Self {
            success: true,
            content: content.into(),
            data: Some(data),
            error: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a failed output
    pub fn failure(error: impl Into<String>) -> Self {
        let error_msg = error.into();
        Self {
            success: false,
            content: String::new(),
            data: None,
            error: Some(error_msg),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Wrapper for MCP tools to implement UnifiedTool
pub struct McpToolWrapper {
    /// MCP client
    client: Arc<McpClient>,
    /// Server name
    server: String,
    /// Tool definition
    tool: McpTool,
}

impl McpToolWrapper {
    /// Create a new MCP tool wrapper
    pub fn new(client: Arc<McpClient>, server: String, tool: McpTool) -> Self {
        Self {
            client,
            server,
            tool,
        }
    }
}

#[async_trait]
impl UnifiedTool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.tool.name
    }

    fn description(&self) -> &str {
        &self.tool.description
    }

    fn input_schema(&self) -> &Value {
        &self.tool.input_schema
    }

    fn source(&self) -> ToolSource {
        ToolSource::Mcp {
            server: self.server.clone(),
        }
    }

    async fn execute(&self, input: Value) -> UnifiedResult<UnifiedOutput> {
        let result = self
            .client
            .call_tool(&self.server, &self.tool.name, input)
            .await?;

        let content = result
            .content
            .iter()
            .filter_map(|c| c.as_text())
            .collect::<Vec<_>>()
            .join("\n");

        if result.is_error {
            Ok(UnifiedOutput::failure(content))
        } else {
            Ok(UnifiedOutput::success(content))
        }
    }
}

/// Wrapper for native skills to implement UnifiedTool
pub struct SkillWrapper<E: ToolExecutor + Send + Sync> {
    /// Skill definition
    skill: Skill,
    /// Skill executor
    executor: Arc<SkillExecutor<E>>,
    /// Cached default schema
    default_schema: Value,
}

impl<E: ToolExecutor + Send + Sync> SkillWrapper<E> {
    /// Create a new skill wrapper
    pub fn new(skill: Skill, executor: Arc<SkillExecutor<E>>) -> Self {
        Self {
            skill,
            executor,
            default_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }
}

#[async_trait]
impl<E: ToolExecutor + Send + Sync + 'static> UnifiedTool for SkillWrapper<E> {
    fn name(&self) -> &str {
        &self.skill.name
    }

    fn description(&self) -> &str {
        &self.skill.description
    }

    fn input_schema(&self) -> &Value {
        self.skill
            .input_schema
            .as_ref()
            .unwrap_or(&self.default_schema)
    }

    fn source(&self) -> ToolSource {
        ToolSource::Native
    }

    async fn execute(&self, input: Value) -> UnifiedResult<UnifiedOutput> {
        // Convert input to variables map
        let variables: HashMap<String, Value> = if let Some(obj) = input.as_object() {
            obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        } else {
            HashMap::new()
        };

        let result = self.executor.execute(&self.skill, &variables).await?;

        if result.success {
            let content = result
                .step_results
                .iter()
                .filter_map(|r| r.output.as_ref())
                .filter_map(|o| o.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            Ok(UnifiedOutput::success(content))
        } else {
            let error = result.error.unwrap_or_else(|| "Unknown error".to_string());
            Ok(UnifiedOutput::failure(error))
        }
    }
}

/// Unified tool registry
///
/// Combines native skills, MCP tools, and built-in tools into a single registry.
pub struct UnifiedRegistry {
    /// Tools by name
    tools: HashMap<String, Arc<dyn UnifiedTool>>,
}

impl UnifiedRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn UnifiedTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn UnifiedTool>> {
        self.tools.get(name).cloned()
    }

    /// List all tool names
    pub fn list_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// List all tools
    pub fn list_all(&self) -> Vec<Arc<dyn UnifiedTool>> {
        self.tools.values().cloned().collect()
    }

    /// Get tools by source
    pub fn list_by_source(&self, source: &ToolSource) -> Vec<Arc<dyn UnifiedTool>> {
        self.tools
            .values()
            .filter(|t| &t.source() == source)
            .cloned()
            .collect()
    }

    /// Register all MCP tools from a client
    pub async fn register_mcp_tools(&mut self, client: Arc<McpClient>) -> UnifiedResult<usize> {
        let tools = client.list_tools().await?;
        let mut count = 0;

        for (server, tool) in tools {
            let wrapper = McpToolWrapper::new(client.clone(), server, tool);
            let prefixed_name = format!("mcp_{}_{}", wrapper.server, wrapper.tool.name);
            self.tools.insert(prefixed_name, Arc::new(wrapper));
            count += 1;
        }

        Ok(count)
    }

    /// Convert tools to cratos-tools ToolDefinition format
    pub fn to_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| {
                let category = match tool.source() {
                    ToolSource::Native => cratos_tools::ToolCategory::Utility,
                    ToolSource::Mcp { .. } => cratos_tools::ToolCategory::External,
                    ToolSource::Builtin => cratos_tools::ToolCategory::Utility,
                };

                ToolDefinition {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    parameters: tool.input_schema().clone(),
                    risk_level: cratos_tools::RiskLevel::Medium,
                    category,
                    enabled: true,
                    required_capabilities: vec![],
                }
            })
            .collect()
    }
}

impl Default for UnifiedRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_output_success() {
        let output = UnifiedOutput::success("Hello, world!");
        assert!(output.success);
        assert_eq!(output.content, "Hello, world!");
        assert!(output.error.is_none());
    }

    #[test]
    fn test_unified_output_failure() {
        let output = UnifiedOutput::failure("Something went wrong");
        assert!(!output.success);
        assert!(output.content.is_empty());
        assert_eq!(output.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_tool_source_display() {
        assert_eq!(ToolSource::Native.to_string(), "native");
        assert_eq!(
            ToolSource::Mcp {
                server: "filesystem".to_string()
            }
            .to_string(),
            "mcp:filesystem"
        );
        assert_eq!(ToolSource::Builtin.to_string(), "builtin");
    }

    #[test]
    fn test_unified_registry() {
        let registry = UnifiedRegistry::new();
        assert!(registry.list_names().is_empty());
    }
}
