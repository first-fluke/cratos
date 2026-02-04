//! Tool types for LLM function calling
//!
//! This module defines the types used for LLM tool/function calling capabilities.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

/// Tool definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON schema for parameters
    pub parameters: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new tool definition
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// A tool call requested by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call
    pub id: String,
    /// Tool name
    pub name: String,
    /// Arguments as JSON string
    pub arguments: String,
}

impl ToolCall {
    /// Parse arguments as a typed value
    pub fn parse_arguments<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.arguments).map_err(|e| Error::InvalidResponse(e.to_string()))
    }
}

/// Tool choice strategy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoice {
    /// Let the model decide
    #[default]
    Auto,
    /// Don't use tools
    None,
    /// Force a specific tool
    Required,
    /// Use a specific tool by name
    Tool(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let tool = ToolDefinition::new(
            "get_weather",
            "Get the current weather",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }),
        );

        assert_eq!(tool.name, "get_weather");
        assert_eq!(tool.description, "Get the current weather");
    }

    #[test]
    fn test_tool_call_parse_arguments() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
            arguments: r#"{"location": "Seoul"}"#.to_string(),
        };

        #[derive(Deserialize)]
        struct Args {
            location: String,
        }

        let args: Args = tool_call.parse_arguments().unwrap();
        assert_eq!(args.location, "Seoul");
    }

    #[test]
    fn test_tool_choice_default() {
        let choice = ToolChoice::default();
        assert!(matches!(choice, ToolChoice::Auto));
    }
}
