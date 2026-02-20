use crate::providers::ollama::types::{OllamaFunction, OllamaMessage, OllamaTool};
use crate::router::{Message, MessageRole, ToolDefinition};

/// Convert messages to Ollama format
pub(crate) fn convert_messages(messages: &[Message]) -> Vec<OllamaMessage> {
    messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };

            OllamaMessage {
                role: role.to_string(),
                content: msg.content.clone(),
                tool_calls: None,
            }
        })
        .collect()
}

/// Convert tool definitions to Ollama format
pub(crate) fn convert_tools(tools: &[ToolDefinition]) -> Vec<OllamaTool> {
    tools
        .iter()
        .map(|tool| OllamaTool {
            r#type: "function".to_string(),
            function: OllamaFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        })
        .collect()
}
