//! Error patterns for categorization

use super::category::FailureCategory;
use std::collections::HashMap;

/// Pattern for matching errors
#[derive(Debug, Clone)]
pub struct ErrorPattern {
    /// Patterns to match in error message (case-insensitive)
    pub keywords: Vec<&'static str>,
    /// Minimum number of keywords that must match
    pub min_matches: usize,
    /// Boost to confidence if matched
    pub confidence_boost: f32,
}

/// Initialize all error patterns
pub fn init_patterns() -> HashMap<FailureCategory, Vec<ErrorPattern>> {
    let mut patterns = HashMap::new();

    // Permission patterns
    patterns.insert(
        FailureCategory::Permission,
        vec![
            ErrorPattern {
                keywords: vec!["permission denied", "access denied", "forbidden"],
                min_matches: 1,
                confidence_boost: 0.9,
            },
            ErrorPattern {
                keywords: vec!["eacces", "eperm", "not permitted"],
                min_matches: 1,
                confidence_boost: 0.85,
            },
        ],
    );

    // Authentication patterns
    patterns.insert(
        FailureCategory::Authentication,
        vec![
            ErrorPattern {
                keywords: vec!["unauthorized", "401", "invalid token"],
                min_matches: 1,
                confidence_boost: 0.9,
            },
            ErrorPattern {
                keywords: vec!["authentication", "api key", "credential"],
                min_matches: 2,
                confidence_boost: 0.8,
            },
            ErrorPattern {
                keywords: vec!["expired", "token", "refresh"],
                min_matches: 2,
                confidence_boost: 0.75,
            },
        ],
    );

    // Network patterns
    patterns.insert(
        FailureCategory::Network,
        vec![
            ErrorPattern {
                keywords: vec!["connection refused", "network unreachable"],
                min_matches: 1,
                confidence_boost: 0.95,
            },
            ErrorPattern {
                keywords: vec!["dns", "resolve", "lookup"],
                min_matches: 2,
                confidence_boost: 0.85,
            },
            ErrorPattern {
                keywords: vec!["econnrefused", "enetunreach", "ehostunreach"],
                min_matches: 1,
                confidence_boost: 0.9,
            },
        ],
    );

    // Rate limit patterns
    patterns.insert(
        FailureCategory::RateLimit,
        vec![
            ErrorPattern {
                keywords: vec!["rate limit", "too many requests", "429"],
                min_matches: 1,
                confidence_boost: 0.95,
            },
            ErrorPattern {
                keywords: vec!["quota", "exceeded", "throttle"],
                min_matches: 2,
                confidence_boost: 0.8,
            },
        ],
    );

    // Not found patterns
    patterns.insert(
        FailureCategory::NotFound,
        vec![
            ErrorPattern {
                keywords: vec!["not found", "404", "no such file"],
                min_matches: 1,
                confidence_boost: 0.9,
            },
            ErrorPattern {
                keywords: vec!["enoent", "does not exist", "cannot find"],
                min_matches: 1,
                confidence_boost: 0.85,
            },
        ],
    );

    // Timeout patterns
    patterns.insert(
        FailureCategory::Timeout,
        vec![
            ErrorPattern {
                keywords: vec!["timeout", "timed out", "deadline exceeded"],
                min_matches: 1,
                confidence_boost: 0.9,
            },
            ErrorPattern {
                keywords: vec!["etimedout", "operation timed out"],
                min_matches: 1,
                confidence_boost: 0.95,
            },
        ],
    );

    // Invalid input patterns
    patterns.insert(
        FailureCategory::InvalidInput,
        vec![
            ErrorPattern {
                keywords: vec!["invalid", "argument", "parameter"],
                min_matches: 2,
                confidence_boost: 0.7,
            },
            ErrorPattern {
                keywords: vec!["malformed", "parse error", "syntax error"],
                min_matches: 1,
                confidence_boost: 0.85,
            },
            ErrorPattern {
                keywords: vec!["missing required", "required field"],
                min_matches: 1,
                confidence_boost: 0.9,
            },
        ],
    );

    // Service unavailable patterns
    patterns.insert(
        FailureCategory::ServiceUnavailable,
        vec![
            ErrorPattern {
                keywords: vec!["service unavailable", "503", "maintenance"],
                min_matches: 1,
                confidence_boost: 0.9,
            },
            ErrorPattern {
                keywords: vec!["temporarily unavailable", "try again later"],
                min_matches: 1,
                confidence_boost: 0.85,
            },
        ],
    );

    // Configuration patterns
    patterns.insert(
        FailureCategory::Configuration,
        vec![
            ErrorPattern {
                keywords: vec!["config", "environment variable", "not set"],
                min_matches: 2,
                confidence_boost: 0.8,
            },
            ErrorPattern {
                keywords: vec!["invalid configuration", "missing configuration"],
                min_matches: 1,
                confidence_boost: 0.9,
            },
        ],
    );

    // Resource exhausted patterns
    patterns.insert(
        FailureCategory::ResourceExhausted,
        vec![
            ErrorPattern {
                keywords: vec!["disk full", "no space left", "enospc"],
                min_matches: 1,
                confidence_boost: 0.95,
            },
            ErrorPattern {
                keywords: vec!["out of memory", "memory exhausted", "enomem"],
                min_matches: 1,
                confidence_boost: 0.95,
            },
            ErrorPattern {
                keywords: vec!["too many open files", "emfile", "enfile"],
                min_matches: 1,
                confidence_boost: 0.9,
            },
        ],
    );

    patterns
}
