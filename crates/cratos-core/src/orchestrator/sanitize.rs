//! Sanitization and helper functions
//!
//! Contains utility functions for:
//! - Error message sanitization
//! - Session memory sanitization
//! - Tool refusal detection
//! - Response sanitization
//! - Fallback eligibility checking

/// H6: Strip absolute paths from error messages shown to users.
/// Security keywords (blocked, denied, etc.) are preserved.
pub fn sanitize_error_for_user(error: &str) -> String {
    use regex::Regex;

    static PATH_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = PATH_RE.get_or_init(|| Regex::new(r"(/[a-zA-Z0-9_./-]+)").unwrap());

    re.replace_all(error, "[PATH]").to_string()
}

/// M2: Sanitize text destined for session memory to prevent prompt injection
/// via square-bracket instructions (e.g. `[SYSTEM: ignore previous instructions]`).
#[allow(dead_code)]
pub fn sanitize_for_session_memory(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '[' | ']')).collect()
}

/// Check if an error message indicates an authentication or permission problem.
pub fn is_auth_or_permission_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("authentication")
        || lower.contains("permission")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("unauthenticated")
}

/// Check if an LLM error is eligible for automatic fallback to a secondary provider.
pub fn is_fallback_eligible(e: &crate::error::Error) -> bool {
    matches!(
        e,
        crate::error::Error::Llm(cratos_llm::Error::RateLimit)
            | crate::error::Error::Llm(cratos_llm::Error::ServerError(_))
            | crate::error::Error::Llm(cratos_llm::Error::Network(_))
            | crate::error::Error::Llm(cratos_llm::Error::Timeout(_))
    ) || matches!(
        e,
        crate::error::Error::Llm(cratos_llm::Error::Api(msg)) if is_auth_or_permission_error(msg)
    )
}

/// Detect if the model's first response is a refusal to use tools.
///
/// Called only on the first iteration when the model responds with text
/// instead of tool calls. Returns true to nudge the model into using tools,
/// false to accept the response as a valid knowledge answer.
///
/// Heuristics:
/// - Long responses (200+ chars) are likely genuine knowledge answers
/// - Responses with code blocks, URLs, or lists are likely substantive
/// - Short vague responses are likely refusals to act
pub fn is_tool_refusal(content: &str) -> bool {
    // Long responses are likely genuine knowledge answers
    if content.chars().count() > 200 {
        return false;
    }
    // Structural markers indicate substantive content, not a refusal
    if content.contains("```") || content.contains("http://") || content.contains("https://") {
        return false;
    }
    // Numbered or bulleted lists suggest a real answer
    if content.contains("\n- ") || content.contains("\n1.") || content.contains("\n* ") {
        return false;
    }
    // Otherwise, treat as a refusal — nudge the model to use tools
    true
}

/// Detect fake tool-use text in responses.
///
/// When a model refuses to call tools, it sometimes generates text that
/// mimics tool call results (e.g. `[Used 1 tool: browser:OK]`) instead
/// of actually invoking tools. If saved to the session, this pattern
/// teaches the model to continue faking tool use in future turns.
pub fn is_fake_tool_use_text(text: &str) -> bool {
    // Pattern: "[Used N tool..." — our own tool summary format, but
    // produced by the model as text instead of actual tool calls.
    // Also detect common LLM-generated fake markers.
    let trimmed = text.trim();
    trimmed.starts_with("[Used ")
        || trimmed.starts_with("[Tool ")
        || trimmed.starts_with("[도구 ")
        || (trimmed.contains(":OK]") && trimmed.len() < 100)
}

/// Sanitize LLM response before sending to users.
///
/// Weak models sometimes generate XML-like tags (e.g. `<tool_response>`) in their text output
/// instead of using the function calling API properly. This strips those artifacts.
pub fn sanitize_response(text: &str) -> String {
    use regex::Regex;

    // Lazy-init compiled regex patterns
    static PATTERNS: std::sync::OnceLock<Vec<Regex>> = std::sync::OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            // <tool_response>...</tool_response> and similar tags
            Regex::new(r"(?s)</?(?:tool_response|tool_call|function_call|function_response|system|thinking)>").unwrap(),
            // JSON blocks that look like raw tool output: {"key": ...}
            // Only strip if preceded by a tag-like marker
            Regex::new(r"(?s)<tool_response>\s*\{[^}]*\}\s*</tool_response>").unwrap(),
        ]
    });

    let mut result = text.to_string();
    for pat in patterns {
        result = pat.replace_all(&result, "").to_string();
    }

    // Clean up excessive blank lines left behind
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    result.trim().to_string()
}

/// Build a user-friendly fallback response when tool execution fails.
///
/// Analyzes tool call records to generate appropriate error messages:
/// - If all failures are security blocks, shows security policy message
/// - Otherwise, shows detailed error information
pub fn build_fallback_response(
    tool_call_records: &[crate::orchestrator::types::ToolCallRecord],
) -> String {
    let failed: Vec<&str> = tool_call_records
        .iter()
        .filter(|r| !r.success)
        .map(|r| r.tool_name.as_str())
        .collect();

    if failed.is_empty() {
        return "요청을 처리하는 중 응답 생성에 실패했습니다. 다시 시도해주세요.".to_string();
    }

    let errors: Vec<String> = tool_call_records
        .iter()
        .filter(|r| !r.success)
        .map(|r| {
            r.output
                .get("stderr")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .or_else(|| r.output.get("error").and_then(|v| v.as_str()))
                .unwrap_or("unknown error")
                .to_string()
        })
        .collect();

    // Deduplicate error messages
    let mut unique_errors: Vec<String> = Vec::new();
    for e in &errors {
        if !unique_errors.iter().any(|u| u == e) {
            unique_errors.push(e.clone());
        }
    }

    // Check if all errors are security blocks
    let all_security = unique_errors.iter().all(|e| {
        let lower = e.to_lowercase();
        lower.contains("blocked")
            || lower.contains("denied")
            || lower.contains("forbidden")
            || lower.contains("restricted")
            || lower.contains("not allowed")
            || lower.contains("unauthorized")
    });

    if all_security {
        let reasons: String = unique_errors
            .iter()
            .map(|e| {
                let short: String = e.chars().take(120).collect();
                format!("- {}", short)
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "보안 정책에 의해 해당 명령어가 차단되었습니다.\n{}\n\n안전한 대체 도구(http_get, http_post 등)를 사용해주세요.",
            reasons
        )
    } else {
        let detail: String = unique_errors
            .iter()
            .map(|e| {
                let short: String = e.chars().take(100).collect();
                format!("- {}", sanitize_error_for_user(&short))
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "도구 실행에 실패했습니다:\n{}\n\n다른 방법으로 시도하거나 명령을 수정해 다시 요청해주세요.",
            detail
        )
    }
}
