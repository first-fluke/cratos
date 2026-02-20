use crate::router::{Message, MessageRole, ToolChoice, ToolDefinition};
use super::types::{
    AnthropicContent, AnthropicMessage, AnthropicTool, AnthropicToolChoice, ContentBlock,
};

/// Convert our message to Anthropic format, returning system message separately
pub(crate) fn convert_messages(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
    let mut system_parts = Vec::new();
    let mut anthropic_messages = Vec::new();

    for msg in messages {
        match msg.role {
            MessageRole::System => {
                // Accumulate system messages
                if !msg.content.is_empty() {
                    system_parts.push(msg.content.clone());
                }
            }
            MessageRole::User => {
                anthropic_messages.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: AnthropicContent::Text(msg.content.clone()),
                });
            }
            MessageRole::Assistant => {
                anthropic_messages.push(AnthropicMessage {
                    role: "assistant".to_string(),
                    content: AnthropicContent::Text(msg.content.clone()),
                });
            }
            MessageRole::Tool => {
                if let Some(tool_call_id) = &msg.tool_call_id {
                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Blocks(vec![ContentBlock::ToolResult {
                            tool_use_id: tool_call_id.clone(),
                            content: msg.content.clone(),
                            is_error: None,
                        }]),
                    });
                }
            }
        }
    }

    let system_message = if !system_parts.is_empty() {
        Some(system_parts.join("\n\n"))
    } else {
        None
    };

    (system_message, anthropic_messages)
}

/// Convert tool definition to Anthropic format
pub(crate) fn convert_tool(tool: &ToolDefinition) -> AnthropicTool {
    AnthropicTool {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: tool.parameters.clone(),
    }
}

/// Convert tool choice to Anthropic format
pub(crate) fn convert_tool_choice(choice: &ToolChoice) -> Option<AnthropicToolChoice> {
    match choice {
        ToolChoice::Auto => Some(AnthropicToolChoice::Auto),
        ToolChoice::None => None, // Anthropic doesn't have a "none" option, we just don't send tools
        ToolChoice::Required => Some(AnthropicToolChoice::Any),
        ToolChoice::Tool(name) => Some(AnthropicToolChoice::Tool { name: name.clone() }),
    }
}
