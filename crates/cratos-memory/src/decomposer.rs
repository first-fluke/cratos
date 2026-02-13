//! Turn decomposer — converts LLM messages into graph turns.
//!
//! Rules:
//! - User message → 1 turn
//! - Assistant message → 1 turn (tool_calls summarised in `summary`)
//! - Tool result → merged into preceding Assistant turn (no separate turn)
//! - System → skipped

use crate::types::{Turn, TurnRole};
use cratos_llm::{Message, MessageRole};
use chrono::Utc;
use uuid::Uuid;

/// Maximum characters kept in the summary field.
const SUMMARY_MAX_CHARS: usize = 250;

/// Maximum characters preserved from a tool result when merging into an assistant turn.
/// Balances retaining enough context for entity extraction vs keeping turns compact.
const TOOL_RESULT_PREVIEW_CHARS: usize = 100;

/// Decompose a list of messages into turns.
///
/// `session_id` tags every produced turn. Already-indexed turns
/// (those with `turn_index <= skip_before`) are not emitted.
pub fn decompose(
    session_id: &str,
    messages: &[Message],
    skip_before: Option<u32>,
) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    let mut turn_index: u32 = 0;

    let mut i = 0;
    while i < messages.len() {
        let msg = &messages[i];

        match msg.role {
            MessageRole::System => {
                // Skip system messages
                i += 1;
                continue;
            }
            MessageRole::User => {
                if skip_before.is_some_and(|s| turn_index <= s) {
                    turn_index += 1;
                    i += 1;
                    continue;
                }
                let summary = build_summary(&msg.content, &[]);
                let token_count = estimate_tokens(&msg.content);
                turns.push(Turn {
                    id: Uuid::new_v4().to_string(),
                    session_id: session_id.into(),
                    role: TurnRole::User,
                    content: msg.content.clone(),
                    summary,
                    turn_index,
                    token_count,
                    created_at: Utc::now(),
                });
                turn_index += 1;
                i += 1;
            }
            MessageRole::Assistant => {
                // Collect tool names from this assistant message
                let tool_names: Vec<&str> = msg
                    .tool_calls
                    .iter()
                    .map(|tc| tc.name.as_str())
                    .collect();

                // Merge any immediately following Tool messages
                let mut merged_content = msg.content.clone();
                let mut j = i + 1;
                while j < messages.len() && messages[j].role == MessageRole::Tool {
                    // Append a short note about the tool result
                    let tool_msg = &messages[j];
                    let result_preview = truncate_safe(&tool_msg.content, TOOL_RESULT_PREVIEW_CHARS);
                    let tool_name = tool_msg.name.as_deref().unwrap_or("tool");
                    merged_content.push_str(&format!(
                        "\n[{tool_name} result: {result_preview}]"
                    ));
                    j += 1;
                }

                if skip_before.is_some_and(|s| turn_index <= s) {
                    turn_index += 1;
                    i = j;
                    continue;
                }

                let summary = build_summary(&msg.content, &tool_names);
                let token_count = estimate_tokens(&merged_content);
                turns.push(Turn {
                    id: Uuid::new_v4().to_string(),
                    session_id: session_id.into(),
                    role: TurnRole::Assistant,
                    content: merged_content,
                    summary,
                    turn_index,
                    token_count,
                    created_at: Utc::now(),
                });
                turn_index += 1;
                i = j;
            }
            MessageRole::Tool => {
                // Orphan tool message (no preceding assistant) — skip
                i += 1;
            }
        }
    }

    turns
}

/// Build a summary for embedding: first N chars + tool names.
fn build_summary(content: &str, tool_names: &[&str]) -> String {
    let mut summary = truncate_safe(content, SUMMARY_MAX_CHARS).to_string();
    if !tool_names.is_empty() {
        summary.push_str(" [tools: ");
        summary.push_str(&tool_names.join(", "));
        summary.push(']');
    }
    summary
}

/// Rough token estimate (~4 chars per token for English, ~2 for CJK).
fn estimate_tokens(text: &str) -> u32 {
    // Simple heuristic: count chars, divide by 3.5
    let chars = text.chars().count();
    ((chars as f64 / 3.5).ceil() as u32).max(1)
}

/// Safely truncate a string at char boundary.
fn truncate_safe(text: &str, max_len: usize) -> &str {
    if text.len() <= max_len {
        return text;
    }
    let end = text
        .char_indices()
        .take_while(|(i, _)| *i < max_len)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use cratos_llm::{Message, ToolCall};

    #[test]
    fn test_basic_decomposition() {
        let messages = vec![
            Message::system("You are an assistant."),
            Message::user("Hello!"),
            Message::assistant("Hi there!"),
            Message::user("How are you?"),
        ];

        let turns = decompose("s1", &messages, None);
        assert_eq!(turns.len(), 3); // system skipped
        assert_eq!(turns[0].role, TurnRole::User);
        assert_eq!(turns[0].content, "Hello!");
        assert_eq!(turns[0].turn_index, 0);
        assert_eq!(turns[1].role, TurnRole::Assistant);
        assert_eq!(turns[1].turn_index, 1);
        assert_eq!(turns[2].role, TurnRole::User);
        assert_eq!(turns[2].turn_index, 2);
    }

    #[test]
    fn test_tool_result_merged() {
        let mut assistant_msg = Message::assistant("Let me check.");
        assistant_msg.tool_calls = vec![ToolCall {
            id: "tc1".into(),
            name: "web_search".into(),
            arguments: "{}".into(),
            thought_signature: None,
        }];
        let tool_msg = Message {
            role: MessageRole::Tool,
            content: "Search results here".into(),
            tool_call_id: Some("tc1".into()),
            name: Some("web_search".into()),
            tool_calls: vec![],
            images: vec![],
        };

        let messages = vec![
            Message::user("Search for Rust"),
            assistant_msg,
            tool_msg,
        ];

        let turns = decompose("s1", &messages, None);
        assert_eq!(turns.len(), 2); // user + assistant (tool merged)
        assert!(turns[1].content.contains("[web_search result:"));
        assert!(turns[1].summary.contains("[tools: web_search]"));
    }

    #[test]
    fn test_skip_before() {
        let messages = vec![
            Message::user("old message"),
            Message::assistant("old response"),
            Message::user("new message"),
        ];

        let turns = decompose("s1", &messages, Some(1));
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].content, "new message");
        assert_eq!(turns[0].turn_index, 2);
    }

    #[test]
    fn test_empty_messages() {
        let turns = decompose("s1", &[], None);
        assert!(turns.is_empty());
    }

    #[test]
    fn test_truncate_safe_multibyte() {
        let text = "한글 테스트입니다";
        let truncated = truncate_safe(text, 10);
        // Should not panic, and be valid UTF-8
        assert!(truncated.len() <= 10);
        assert!(truncated.is_char_boundary(truncated.len()));
    }

    #[test]
    fn test_estimate_tokens() {
        assert!(estimate_tokens("hello world") > 0);
        assert!(estimate_tokens("한글") > 0);
    }
}
