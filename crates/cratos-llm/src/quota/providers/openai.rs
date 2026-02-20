use crate::quota::{header_u64, PartialQuotaState};
use chrono::{DateTime, Utc};
use reqwest::header::HeaderMap;

/// Parse OpenAI-compatible rate limit headers.
pub(crate) fn parse_openai_headers(headers: &HeaderMap) -> PartialQuotaState {
    PartialQuotaState {
        requests_limit: header_u64(headers, "x-ratelimit-limit-requests"),
        requests_remaining: header_u64(headers, "x-ratelimit-remaining-requests"),
        tokens_limit: header_u64(headers, "x-ratelimit-limit-tokens"),
        tokens_remaining: header_u64(headers, "x-ratelimit-remaining-tokens"),
        // OpenAI uses durations like "6m0s" or ISO 8601 timestamps
        reset_at: header_openai_reset(headers, "x-ratelimit-reset-requests")
            .or_else(|| header_openai_reset(headers, "x-ratelimit-reset-tokens")),
    }
}

/// Fallback parser that tries both header formats.
pub(crate) fn parse_generic_headers(headers: &HeaderMap) -> PartialQuotaState {
    let anthropic = super::anthropic::parse_anthropic_headers(headers);
    if anthropic.has_data() {
        return anthropic;
    }
    parse_openai_headers(headers)
}

/// Parse OpenAI-style reset header.
fn header_openai_reset(headers: &HeaderMap, name: &str) -> Option<DateTime<Utc>> {
    let val = headers.get(name).and_then(|v| v.to_str().ok())?;

    // Try ISO 8601 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(val) {
        return Some(dt.with_timezone(&Utc));
    }

    // Parse Go-style duration: "6m0s", "1m30.5s", "200ms", "45s"
    if let Some(secs) = parse_go_duration(val) {
        return Some(Utc::now() + chrono::Duration::milliseconds((secs * 1000.0) as i64));
    }

    None
}

/// Parse a Go-style duration string into total seconds.
fn parse_go_duration(s: &str) -> Option<f64> {
    let mut total_secs = 0.0_f64;
    let mut num_buf = String::new();
    let mut chars = s.chars().peekable();
    let mut parsed_any = false;

    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() || ch == '.' {
            num_buf.push(ch);
            chars.next();
        } else if ch == 'h' {
            chars.next();
            let val: f64 = num_buf.parse().ok()?;
            total_secs += val * 3600.0;
            num_buf.clear();
            parsed_any = true;
        } else if ch == 'm' {
            chars.next();
            // Check for "ms"
            if chars.peek() == Some(&'s') {
                chars.next();
                let val: f64 = num_buf.parse().ok()?;
                total_secs += val / 1000.0;
            } else {
                let val: f64 = num_buf.parse().ok()?;
                total_secs += val * 60.0;
            }
            num_buf.clear();
            parsed_any = true;
        } else if ch == 's' {
            chars.next();
            let val: f64 = num_buf.parse().ok()?;
            total_secs += val;
            num_buf.clear();
            parsed_any = true;
        } else {
            // Unknown character
            return None;
        }
    }

    if parsed_any {
        Some(total_secs)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn test_parse_go_duration_minutes_seconds() {
        assert!((parse_go_duration("6m0s").unwrap() - 360.0).abs() < 0.001);
        assert!((parse_go_duration("1m30s").unwrap() - 90.0).abs() < 0.001);
        assert!((parse_go_duration("1m30.5s").unwrap() - 90.5).abs() < 0.001);
    }

    #[test]
    fn test_parse_go_duration_milliseconds() {
        assert!((parse_go_duration("200ms").unwrap() - 0.2).abs() < 0.001);
        assert!((parse_go_duration("1500ms").unwrap() - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_parse_go_duration_seconds() {
        assert!((parse_go_duration("45s").unwrap() - 45.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_go_duration_complex() {
        assert!((parse_go_duration("1h2m3s").unwrap() - 3723.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_go_duration_invalid() {
        assert!(parse_go_duration("abc").is_none());
        assert!(parse_go_duration("").is_none());
    }

    #[test]
    fn test_parse_openai_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-limit-requests", HeaderValue::from_static("60"));
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("12"),
        );
        headers.insert(
            "x-ratelimit-limit-tokens",
            HeaderValue::from_static("15000"),
        );
        headers.insert(
            "x-ratelimit-remaining-tokens",
            HeaderValue::from_static("8500"),
        );
        headers.insert(
            "x-ratelimit-reset-requests",
            HeaderValue::from_static("2m14s"),
        );

        let partial = parse_openai_headers(&headers);
        assert_eq!(partial.requests_limit, Some(60));
        assert_eq!(partial.requests_remaining, Some(12));
        assert_eq!(partial.tokens_limit, Some(15_000));
        assert_eq!(partial.tokens_remaining, Some(8_500));
        assert!(partial.reset_at.is_some());
    }
}
