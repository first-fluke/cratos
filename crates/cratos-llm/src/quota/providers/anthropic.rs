use crate::quota::{header_datetime, header_u64, PartialQuotaState};
use reqwest::header::HeaderMap;

/// Parse Anthropic rate limit headers.
pub(crate) fn parse_anthropic_headers(headers: &HeaderMap) -> PartialQuotaState {
    PartialQuotaState {
        requests_limit: header_u64(headers, "anthropic-ratelimit-requests-limit"),
        requests_remaining: header_u64(headers, "anthropic-ratelimit-requests-remaining"),
        tokens_limit: header_u64(headers, "anthropic-ratelimit-tokens-limit"),
        tokens_remaining: header_u64(headers, "anthropic-ratelimit-tokens-remaining"),
        // Anthropic uses ISO 8601 for reset times
        reset_at: header_datetime(headers, "anthropic-ratelimit-requests-reset")
            .or_else(|| header_datetime(headers, "anthropic-ratelimit-tokens-reset")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn test_parse_anthropic_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "anthropic-ratelimit-requests-limit",
            HeaderValue::from_static("1000"),
        );
        headers.insert(
            "anthropic-ratelimit-requests-remaining",
            HeaderValue::from_static("987"),
        );
        headers.insert(
            "anthropic-ratelimit-tokens-limit",
            HeaderValue::from_static("100000"),
        );
        headers.insert(
            "anthropic-ratelimit-tokens-remaining",
            HeaderValue::from_static("45200"),
        );
        headers.insert(
            "anthropic-ratelimit-requests-reset",
            HeaderValue::from_static("2026-02-06T15:30:00Z"),
        );

        let partial = parse_anthropic_headers(&headers);
        assert_eq!(partial.requests_limit, Some(1000));
        assert_eq!(partial.requests_remaining, Some(987));
        assert_eq!(partial.tokens_limit, Some(100_000));
        assert_eq!(partial.tokens_remaining, Some(45_200));
        assert!(partial.reset_at.is_some());
    }
}
