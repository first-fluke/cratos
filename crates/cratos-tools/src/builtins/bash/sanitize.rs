//! Output sanitization and ANSI escape handling

use super::constants::SECRET_PATTERNS;
use tracing::warn;

/// Sanitize output by masking secrets and sensitive data
pub(crate) fn sanitize_output(output: &str) -> String {
    let mut result = output.to_string();
    for pattern in SECRET_PATTERNS {
        if result.contains(pattern) {
            warn!(pattern = %pattern, "Secret detected in output, masking");
            let mask_prefix: String = pattern.chars().take(4).collect();
            result = result.replace(pattern, &format!("[MASKED:{}...]", mask_prefix));
        }
    }
    // M1: Detect and mask potential base64-encoded secrets in output
    // Threshold 44 chars (≥ 32 raw bytes encoded), include URL-safe base64 chars
    let mut masked_lines: Vec<String> = Vec::new();
    let mut did_mask = false;
    for line in result.lines() {
        let t = line.trim();
        if t.len() >= 44
            && t.chars().all(|c| {
                c.is_ascii_alphanumeric()
                    || c == '+'
                    || c == '/'
                    || c == '='
                    || c == '-'
                    || c == '_'
            })
        {
            warn!("Base64-encoded data in output masked (len={})", t.len());
            masked_lines.push(format!("[MASKED:BASE64_DATA:{}bytes]", t.len()));
            did_mask = true;
        } else {
            masked_lines.push(line.to_string());
        }
    }
    if did_mask {
        masked_lines.join("\n")
    } else {
        result
    }
}

/// Strip ANSI escape sequences from output.
pub(crate) fn strip_ansi_escapes(s: &str) -> String {
    // Match ANSI CSI sequences: ESC [ ... final_byte
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip ESC sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                              // Consume parameters and intermediate bytes (0x20–0x3F)
                while let Some(&next) = chars.peek() {
                    if next.is_ascii() && (0x20..=0x3F).contains(&(next as u8)) {
                        chars.next();
                    } else {
                        break;
                    }
                }
                // Consume final byte (0x40–0x7E)
                if let Some(&next) = chars.peek() {
                    if next.is_ascii() && (0x40..=0x7E).contains(&(next as u8)) {
                        chars.next();
                    }
                }
            } else if chars.peek() == Some(&']') {
                // OSC sequence: ESC ] ... ST (ESC \ or BEL)
                chars.next(); // consume ']'
                while let Some(c) = chars.next() {
                    if c == '\x07' {
                        break;
                    } // BEL
                    if c == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            // else skip single char after ESC
        } else if c == '\r' {
            // Skip carriage returns (PTY outputs \r\n)
            continue;
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_secret_masking() {
        let output = "data: ghp_ABCdefgh123, key: xoxb-token-here";
        let sanitized = sanitize_output(output);
        assert!(!sanitized.contains("ghp_ABCdefgh123"));
        assert!(!sanitized.contains("xoxb-token-here"));
        assert!(sanitized.contains("[MASKED:"));
    }

    #[test]
    fn test_strip_ansi_escapes() {
        let input = "\x1b[32mHello\x1b[0m World\r\n";
        let result = strip_ansi_escapes(input);
        assert_eq!(result, "Hello World\n");
    }

    #[test]
    fn test_base64_masking() {
        // 44+ chars of pure base64 should be masked
        let long_b64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrst";
        let output = format!("prefix\n{}\nsuffix", long_b64);
        let sanitized = sanitize_output(&output);
        assert!(
            sanitized.contains("[MASKED:BASE64_DATA:"),
            "got: {}",
            sanitized
        );
        assert!(sanitized.contains("prefix"));
        assert!(sanitized.contains("suffix"));
        // Short base64-like string should NOT be masked
        let short = "ABCDEF1234";
        let sanitized2 = sanitize_output(short);
        assert_eq!(sanitized2, short);
    }
}
