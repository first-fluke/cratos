//! Message and tool conversion utilities for Gemini API

use super::schema::strip_unsupported_schema_fields;
use super::types::*;
use crate::router::{Message, MessageRole, ToolChoice, ToolDefinition};

/// Convert messages to Gemini format, returning system instruction separately
pub(crate) fn convert_messages(messages: &[Message]) -> (Option<GeminiContent>, Vec<GeminiContent>) {
    let mut system_instruction = None;
    let mut gemini_contents = Vec::new();

    for msg in messages {
        match msg.role {
            MessageRole::System => {
                system_instruction = Some(GeminiContent {
                    role: None,
                    parts: vec![GeminiPart::Text {
                        text: msg.content.clone(),
                    }],
                });
            }
            MessageRole::User => {
                let mut parts = Vec::new();
                if !msg.content.is_empty() {
                    parts.push(GeminiPart::Text {
                        text: msg.content.clone(),
                    });
                }
                for img in &msg.images {
                    parts.push(GeminiPart::InlineData {
                        inline_data: InlineData {
                            mime_type: img.mime_type.clone(),
                            data: img.base64_data(),
                        },
                    });
                }
                if !parts.is_empty() {
                    gemini_contents.push(GeminiContent {
                        role: Some("user".to_string()),
                        parts,
                    });
                }
            }
            MessageRole::Assistant => {
                let mut parts: Vec<GeminiPart> = Vec::new();
                if !msg.content.is_empty() {
                    parts.push(GeminiPart::Text {
                        text: msg.content.clone(),
                    });
                }
                // Include function calls from assistant's tool_calls
                for tc in &msg.tool_calls {
                    let args = serde_json::from_str(&tc.arguments)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    parts.push(GeminiPart::FunctionCall {
                        function_call: FunctionCall {
                            name: tc.name.clone(),
                            args,
                        },
                        // Gemini 3+ thought signatures must be preserved exactly
                        thought_signature: tc.thought_signature.clone(),
                    });
                }
                if !parts.is_empty() {
                    gemini_contents.push(GeminiContent {
                        role: Some("model".to_string()),
                        parts,
                    });
                }
            }
            MessageRole::Tool => {
                if let Some(tool_name) = &msg.name {
                    // Parse the content as JSON for the response
                    let response_value = serde_json::from_str(&msg.content)
                        .unwrap_or_else(|_| serde_json::json!({"result": msg.content}));

                    let part = GeminiPart::FunctionResponse {
                        function_response: FunctionResponse {
                            name: tool_name.clone(),
                            response: response_value,
                        },
                    };

                    // Gemini requires all FunctionResponse parts in a single user turn
                    // matching the number of FunctionCall parts. Merge consecutive
                    // Tool messages into one GeminiContent.
                    if let Some(last) = gemini_contents.last_mut() {
                        if last.role.as_deref() == Some("user")
                            && last
                                .parts
                                .iter()
                                .all(|p| matches!(p, GeminiPart::FunctionResponse { .. }))
                        {
                            last.parts.push(part);
                        } else {
                            gemini_contents.push(GeminiContent {
                                role: Some("user".to_string()),
                                parts: vec![part],
                            });
                        }
                    } else {
                        gemini_contents.push(GeminiContent {
                            role: Some("user".to_string()),
                            parts: vec![part],
                        });
                    }
                }
            }
        }
    }

    // Gemini 3 thinking models require ALL function calls to carry
    // `thoughtSignature`.  If the conversation history mixes calls from
    // different providers (e.g. after a fallback), drop the function call
    // turns that lack signatures so only consistent turns remain.
    let has_some = gemini_contents.iter().any(|c| {
        c.parts.iter().any(|p| {
            matches!(
                p,
                GeminiPart::FunctionCall {
                    thought_signature: Some(_),
                    ..
                }
            )
        })
    });
    if has_some {
        let before = gemini_contents.len();
        // Remove FunctionCall parts without thought_signature from model turns
        for content in &mut gemini_contents {
            if content.role.as_deref() == Some("model") {
                content.parts.retain(|p| {
                    !matches!(
                        p,
                        GeminiPart::FunctionCall {
                            thought_signature: None,
                            ..
                        }
                    )
                });
            }
        }
        // Remove empty model turns and orphaned FunctionResponse-only user turns
        // that no longer have a matching FunctionCall
        gemini_contents.retain(|c| !c.parts.is_empty());
        let after = gemini_contents.len();
        if before != after {
            tracing::warn!(
                removed = before - after,
                "Dropped content blocks with missing thought_signature to maintain Gemini 3 consistency"
            );
        }
    }

    (system_instruction, gemini_contents)
}

/// Convert tool definitions to Gemini format
pub(crate) fn convert_tools(tools: &[ToolDefinition]) -> Vec<GeminiTool> {
    let declarations: Vec<FunctionDeclaration> = tools
        .iter()
        .map(|tool| {
            let mut params = tool.parameters.clone();
            strip_unsupported_schema_fields(&mut params);
            FunctionDeclaration {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: params,
            }
        })
        .collect();

    vec![GeminiTool {
        function_declarations: declarations,
    }]
}

/// Convert tool choice to Gemini format
pub(crate) fn convert_tool_choice(
    choice: &ToolChoice,
    _tools: &[ToolDefinition],
) -> Option<ToolConfig> {
    match choice {
        ToolChoice::Auto => Some(ToolConfig {
            function_calling_config: FunctionCallingConfig {
                mode: "AUTO".to_string(),
                allowed_function_names: None,
            },
        }),
        ToolChoice::None => Some(ToolConfig {
            function_calling_config: FunctionCallingConfig {
                mode: "NONE".to_string(),
                allowed_function_names: None,
            },
        }),
        ToolChoice::Required => Some(ToolConfig {
            function_calling_config: FunctionCallingConfig {
                mode: "ANY".to_string(),
                allowed_function_names: None,
            },
        }),
        ToolChoice::Tool(name) => Some(ToolConfig {
            function_calling_config: FunctionCallingConfig {
                mode: "ANY".to_string(),
                allowed_function_names: Some(vec![name.clone()]),
            },
        }),
    }
}
