//! Common utilities for channel adapters
//!
//! This module contains shared helper functions used across multiple channel adapters
//! to avoid code duplication (DRY principle).

// ============================================================================
// Logging and Security Constants
// ============================================================================

/// Maximum length of text to log (to prevent sensitive data exposure)
pub const MAX_LOG_TEXT_LENGTH: usize = 50;

/// Maximum length of error message to show to users (longer = likely internal)
pub const MAX_SAFE_ERROR_LENGTH: usize = 100;

// ============================================================================
// Platform Message Length Limits
// ============================================================================

/// Discord message character limit
pub const DISCORD_MESSAGE_LIMIT: usize = 2000;

/// WhatsApp message character limit (recommended split size)
pub const WHATSAPP_MESSAGE_LIMIT: usize = 4096;

/// Patterns that indicate potentially sensitive content
pub const SENSITIVE_PATTERNS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "api-key",
    "bearer",
    "authorization",
    "credential",
    "private",
    "ssh",
    "-----begin",
];

/// Mask potentially sensitive text for logging
///
/// Checks for sensitive patterns and truncates long messages
/// to prevent accidental exposure of sensitive data in logs.
///
/// # Examples
/// ```
/// use cratos_channels::util::mask_for_logging;
///
/// // Sensitive content is redacted
/// assert!(mask_for_logging("my password is secret123").contains("REDACTED"));
///
/// // Normal short messages pass through
/// assert_eq!(mask_for_logging("Hello"), "Hello");
/// ```
#[must_use]
pub fn mask_for_logging(text: &str) -> String {
    let lower = text.to_lowercase();

    // Check for sensitive patterns
    for pattern in SENSITIVE_PATTERNS {
        if lower.contains(pattern) {
            return "[REDACTED - potentially sensitive content]".to_string();
        }
    }

    // Truncate long messages (char boundary safe for multi-byte UTF-8)
    if text.len() > MAX_LOG_TEXT_LENGTH {
        let truncated = match text.char_indices().take_while(|(i, _)| *i < MAX_LOG_TEXT_LENGTH).last() {
            Some((i, c)) => &text[..i + c.len_utf8()],
            None => "",
        };
        format!("{truncated}...[truncated]")
    } else {
        text.to_string()
    }
}

/// Sanitize error messages to avoid exposing internal details
///
/// Removes or masks potentially sensitive information from error messages
/// before showing them to users via messaging channels.
///
/// # Examples
/// ```
/// use cratos_channels::util::sanitize_error_for_user;
///
/// // Auth errors are sanitized
/// let sanitized = sanitize_error_for_user("Invalid token: abc123");
/// assert!(!sanitized.contains("abc123"));
///
/// // Simple safe errors pass through
/// assert_eq!(sanitize_error_for_user("File not found"), "File not found");
/// ```
#[must_use]
pub fn sanitize_error_for_user(error: &str) -> String {
    let lower = error.to_lowercase();

    // Hide authentication-related errors
    if lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
    {
        return "An authentication error occurred. Please check your configuration.".to_string();
    }

    // Hide network errors
    if lower.contains("connection") || lower.contains("timeout") || lower.contains("network") {
        return "A network error occurred. Please try again later.".to_string();
    }

    // Hide database errors
    if lower.contains("database") || lower.contains("sql") || lower.contains("query") {
        return "A database error occurred. Please try again later.".to_string();
    }

    // Hide internal errors (paths, stack traces)
    if error.len() > MAX_SAFE_ERROR_LENGTH || error.contains('/') || error.contains("at ") {
        return "An internal error occurred. Please try again.".to_string();
    }

    // Short, non-sensitive errors can be shown
    error.to_string()
}

// ============================================================================
// Markdown → HTML Conversion (for Telegram)
// ============================================================================

/// Convert standard Markdown (LLM output) to Telegram-compatible HTML.
///
/// Telegram's `ParseMode::Html` supports a limited subset of HTML tags.
/// This function handles the most common Markdown patterns:
/// - `**bold**` / `__bold__` → `<b>bold</b>`
/// - `*italic*` / `_italic_` → `<i>italic</i>`
/// - `` `code` `` → `<code>code</code>`
/// - `` ```lang\nblock\n``` `` → `<pre><code class="language-lang">block</code></pre>`
/// - `~~strike~~` → `<s>strike</s>`
/// - `[text](url)` → `<a href="url">text</a>`
///
/// HTML entities (`&`, `<`, `>`) are escaped first to prevent injection.
///
/// # Examples
/// ```
/// use cratos_channels::util::markdown_to_html;
///
/// assert_eq!(markdown_to_html("**bold**"), "<b>bold</b>");
/// assert_eq!(markdown_to_html("use `code` here"), "use <code>code</code> here");
/// assert_eq!(markdown_to_html("a < b & c > d"), "a &lt; b &amp; c &gt; d");
/// ```
#[must_use]
pub fn markdown_to_html(text: &str) -> String {
    // Phase 1: Extract code blocks to protect them from further processing
    let mut protected: Vec<String> = Vec::new();
    let mut work = String::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("```") {
        // Text before the code block — will be processed later
        work.push_str(&remaining[..start]);

        let after_ticks = &remaining[start + 3..];
        if let Some(end) = after_ticks.find("```") {
            // Extract lang + body
            let block_content = &after_ticks[..end];
            let (lang, body) = match block_content.find('\n') {
                Some(nl) => {
                    let lang = block_content[..nl].trim();
                    let body = &block_content[nl + 1..];
                    (lang, body)
                }
                None => ("", block_content),
            };

            let escaped_body = escape_html(body);
            let placeholder = format!("\x00CODEBLOCK{}\x00", protected.len());
            if lang.is_empty() {
                protected.push(format!("<pre><code>{}</code></pre>", escaped_body));
            } else {
                protected.push(format!(
                    "<pre><code class=\"language-{lang}\">{}</code></pre>",
                    escaped_body
                ));
            }
            work.push_str(&placeholder);
            remaining = &after_ticks[end + 3..];
        } else {
            // Unclosed code block — treat as plain text
            work.push_str("```");
            remaining = after_ticks;
        }
    }
    work.push_str(remaining);

    // Phase 2: Escape HTML entities in non-code-block text
    // We need to escape before applying markdown, but inline code also needs protection
    let mut rest = work.as_str();

    // Protect inline code first
    let mut inline_codes: Vec<String> = Vec::new();
    let mut temp = String::new();
    while let Some(start) = rest.find('`') {
        temp.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('`') {
            let code = &after[..end];
            let placeholder = format!("\x00INLINECODE{}\x00", inline_codes.len());
            inline_codes.push(format!("<code>{}</code>", escape_html(code)));
            temp.push_str(&placeholder);
            rest = &after[end + 1..];
        } else {
            temp.push('`');
            rest = after;
        }
    }
    temp.push_str(rest);

    // Escape HTML in non-code portions
    let mut result = escape_html(&temp);

    // Phase 3: Apply Markdown → HTML conversions
    // Bold: **text** or __text__
    result = replace_pattern(&result, "**", "<b>", "</b>");
    result = replace_pattern(&result, "__", "<b>", "</b>");
    // Italic: *text* or _text_ (after bold is handled)
    result = replace_pattern(&result, "*", "<i>", "</i>");
    result = replace_pattern(&result, "_", "<i>", "</i>");
    // Strikethrough: ~~text~~
    result = replace_pattern(&result, "~~", "<s>", "</s>");

    // Links: [text](url)
    result = convert_links(&result);

    // Phase 4: Restore inline code placeholders
    for (i, code) in inline_codes.iter().enumerate() {
        let placeholder = format!("\x00INLINECODE{i}\x00");
        // Placeholder was HTML-escaped, so look for escaped version
        let escaped_placeholder = escape_html(&placeholder);
        result = result.replace(&escaped_placeholder, code);
    }

    // Phase 5: Restore code block placeholders
    for (i, block) in protected.iter().enumerate() {
        let placeholder = format!("\x00CODEBLOCK{i}\x00");
        let escaped_placeholder = escape_html(&placeholder);
        result = result.replace(&escaped_placeholder, block);
    }

    result
}

/// Escape HTML special characters: `&`, `<`, `>`
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Replace paired Markdown delimiters with HTML tags.
///
/// Handles `**bold**` → `<b>bold</b>` style patterns.
fn replace_pattern(text: &str, delimiter: &str, open_tag: &str, close_tag: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    let mut open = true;

    while let Some(pos) = rest.find(delimiter) {
        result.push_str(&rest[..pos]);
        if open {
            result.push_str(open_tag);
        } else {
            result.push_str(close_tag);
        }
        open = !open;
        rest = &rest[pos + delimiter.len()..];
    }
    result.push_str(rest);

    // If we have an unclosed tag, the last open_tag was wrong — revert
    if !open {
        // Odd number of delimiters — put the last delimiter back as-is
        if let Some(pos) = result.rfind(open_tag) {
            let escaped_delim = escape_html(delimiter);
            result.replace_range(pos..pos + open_tag.len(), &escaped_delim);
        }
    }

    result
}

/// Convert Markdown links `[text](url)` to HTML `<a href="url">text</a>`.
fn convert_links(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(bracket_start) = rest.find('[') {
        result.push_str(&rest[..bracket_start]);
        let after_bracket = &rest[bracket_start + 1..];

        if let Some(bracket_end) = after_bracket.find("](") {
            let link_text = &after_bracket[..bracket_end];
            let after_paren = &after_bracket[bracket_end + 2..];

            if let Some(paren_end) = after_paren.find(')') {
                let url = &after_paren[..paren_end];
                result.push_str(&format!("<a href=\"{url}\">{link_text}</a>"));
                rest = &after_paren[paren_end + 1..];
                continue;
            }
        }

        // Not a valid link — keep the bracket
        result.push('[');
        rest = after_bracket;
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_for_logging_sensitive() {
        assert!(mask_for_logging("my password is secret123").contains("REDACTED"));
        assert!(mask_for_logging("API_KEY=sk-1234567890").contains("REDACTED"));
        assert!(mask_for_logging("Bearer eyJhbGciOiJ").contains("REDACTED"));
        assert!(mask_for_logging("-----BEGIN RSA PRIVATE KEY-----").contains("REDACTED"));
    }

    #[test]
    fn test_mask_for_logging_truncate() {
        let long_msg = "a".repeat(100);
        let masked = mask_for_logging(&long_msg);
        assert!(masked.contains("truncated"));
        assert!(masked.len() < long_msg.len());
    }

    #[test]
    fn test_mask_for_logging_truncate_multibyte() {
        // Korean text: each char is 3 bytes, this string exceeds MAX_LOG_TEXT_LENGTH
        let korean_msg = "지금 파일디렉토리 예시로 하나 알려줘봐 그리고 추가로 더 알려줘";
        assert!(korean_msg.len() > MAX_LOG_TEXT_LENGTH);
        // Must not panic on multi-byte boundary
        let masked = mask_for_logging(korean_msg);
        assert!(masked.contains("truncated"));
        // Verify it's valid UTF-8 (would fail at compile time if not, but sanity check)
        assert!(masked.is_char_boundary(0));
    }

    #[test]
    fn test_mask_for_logging_pass_through() {
        assert_eq!(mask_for_logging("Hello, world!"), "Hello, world!");
        assert_eq!(mask_for_logging("요약해줘"), "요약해줘");
    }

    #[test]
    fn test_sanitize_error_auth() {
        let sanitized = sanitize_error_for_user("Invalid token: abc123");
        assert!(!sanitized.contains("abc123"));
        assert!(sanitized.contains("authentication"));
    }

    #[test]
    fn test_sanitize_error_database() {
        let sanitized = sanitize_error_for_user("SQL error: SELECT * FROM users");
        assert!(!sanitized.contains("SELECT"));
        assert!(sanitized.contains("database"));
    }

    #[test]
    fn test_sanitize_error_internal() {
        let sanitized =
            sanitize_error_for_user("Error at /home/user/.config/app/config.json line 42");
        assert!(!sanitized.contains("/home"));
        assert!(sanitized.to_lowercase().contains("internal"));
    }

    #[test]
    fn test_sanitize_error_pass_through() {
        let simple = sanitize_error_for_user("File not found");
        assert_eq!(simple, "File not found");
    }

    // ── markdown_to_html tests ──────────────────────────────────────────

    #[test]
    fn test_html_escape() {
        assert_eq!(markdown_to_html("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn test_bold() {
        assert_eq!(markdown_to_html("**bold**"), "<b>bold</b>");
        assert_eq!(markdown_to_html("__bold__"), "<b>bold</b>");
    }

    #[test]
    fn test_italic() {
        assert_eq!(markdown_to_html("*italic*"), "<i>italic</i>");
        assert_eq!(markdown_to_html("_italic_"), "<i>italic</i>");
    }

    #[test]
    fn test_bold_and_italic() {
        assert_eq!(
            markdown_to_html("**bold** and *italic*"),
            "<b>bold</b> and <i>italic</i>"
        );
    }

    #[test]
    fn test_inline_code() {
        assert_eq!(
            markdown_to_html("use `code` here"),
            "use <code>code</code> here"
        );
    }

    #[test]
    fn test_inline_code_html_escape() {
        assert_eq!(
            markdown_to_html("use `a<b>` here"),
            "use <code>a&lt;b&gt;</code> here"
        );
    }

    #[test]
    fn test_code_block() {
        let input = "before\n```rust\nfn main() {}\n```\nafter";
        let result = markdown_to_html(input);
        assert!(result.contains("<pre><code class=\"language-rust\">"));
        assert!(result.contains("fn main() {}\n</code></pre>"));
        assert!(result.starts_with("before\n"));
        assert!(result.ends_with("\nafter"));
    }

    #[test]
    fn test_code_block_no_lang() {
        let input = "```\nhello\n```";
        let result = markdown_to_html(input);
        assert!(result.contains("<pre><code>"));
        assert!(result.contains("hello\n</code></pre>"));
    }

    #[test]
    fn test_code_block_html_escape() {
        let input = "```\n<div>&</div>\n```";
        let result = markdown_to_html(input);
        assert!(result.contains("&lt;div&gt;&amp;&lt;/div&gt;"));
    }

    #[test]
    fn test_strikethrough() {
        assert_eq!(markdown_to_html("~~strike~~"), "<s>strike</s>");
    }

    #[test]
    fn test_link() {
        assert_eq!(
            markdown_to_html("[Google](https://google.com)"),
            "<a href=\"https://google.com\">Google</a>"
        );
    }

    #[test]
    fn test_unclosed_bold() {
        // Odd number of ** — the closed pair is rendered, orphan degrades gracefully
        let result = markdown_to_html("**bold** and **orphan");
        assert!(result.contains("<b>bold</b>"));
        // The orphan ** is not rendered as bold (no closing **)
        assert!(!result.contains("<b>orphan"));
    }

    #[test]
    fn test_plain_text() {
        assert_eq!(markdown_to_html("hello world"), "hello world");
    }

    #[test]
    fn test_mixed_markdown() {
        let input = "**Title**\n\nHello *world*, use `cmd` and visit [site](https://x.com)";
        let result = markdown_to_html(input);
        assert!(result.contains("<b>Title</b>"));
        assert!(result.contains("<i>world</i>"));
        assert!(result.contains("<code>cmd</code>"));
        assert!(result.contains("<a href=\"https://x.com\">site</a>"));
    }

    #[test]
    fn test_korean_text() {
        assert_eq!(
            markdown_to_html("**안녕하세요** 세계"),
            "<b>안녕하세요</b> 세계"
        );
    }
}
