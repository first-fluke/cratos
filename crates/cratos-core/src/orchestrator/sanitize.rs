//! Sanitization and helper functions
//!
//! Contains utility functions for:
//! - Error message sanitization
//! - Session memory sanitization
//! - Tool refusal detection
//! - Response sanitization
//! - Fallback eligibility checking

/// System prompt for lightweight persona classification via LLM.
pub(crate) const PERSONA_CLASSIFICATION_PROMPT: &str = r#"Classify the user message into the most appropriate persona. Output ONLY the persona name, nothing else.

Personas:
- sindri: software development, coding, API, database, architecture, debugging, implementation
- brok: software development (secondary dev persona, use sindri by default)
- athena: project management, planning, requirements, roadmap, sprint, schedule
- heimdall: QA, testing, security, code review, bug analysis, vulnerability
- mimir: research, investigation, analysis, comparison, documentation, study
- thor: DevOps, deployment, CI/CD, Docker, Kubernetes, infrastructure, server ops
- apollo: UX/UI design, user experience, prototyping, accessibility, wireframe
- odin: product ownership, vision, prioritization, OKR, stakeholder management
- nike: marketing, SNS, social media, growth hacking, SEO, content, campaign, automation, bot, like, comment, tweet
- freya: customer support, CS, help desk, user complaints, FAQ
- hestia: HR, hiring, team management, organization, onboarding
- norns: business analysis, data analysis, metrics, KPI, reporting, forecasting
- tyr: legal, compliance, regulation, privacy, GDPR, terms of service
- cratos: general tasks, greetings, unclear domain, multi-domain, status, weather, casual

Rules:
- Output ONLY the persona name, nothing else
- If the user explicitly names a persona (e.g. "니케", "nike", "아폴로"), use that persona
- If uncertain or multi-domain, output "cratos"
- Understand intent regardless of language (Korean, English, Japanese, etc.)"#;

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
/// A response is classified as a refusal when it is either:
/// - empty, or
/// - very short (<60 chars) and lacks substantive content markers
///   (code backticks, URLs, lists) that would indicate a genuine answer.
///
/// The previous 200-char threshold was too aggressive and incorrectly
/// flagged legitimate short knowledge answers, forcing unnecessary tool
/// calls that wasted iterations and caused timeouts.
pub fn is_tool_refusal(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return true;
    }
    // Substantive content markers: code, URLs, lists → genuine answer
    if trimmed.contains('`')
        || trimmed.contains("http")
        || trimmed.contains("1.")
        || trimmed.contains("- ")
    {
        return false;
    }
    trimmed.len() < 60
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
