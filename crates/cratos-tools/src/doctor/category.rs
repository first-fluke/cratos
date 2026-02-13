//! Failure categories for tool diagnosis

use serde::{Deserialize, Serialize};

/// Categories of tool failures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    /// Permission denied
    Permission,
    /// Invalid or expired token/credential
    Authentication,
    /// Network connectivity issues
    Network,
    /// Rate limit exceeded
    RateLimit,
    /// Invalid path or resource not found
    NotFound,
    /// Timeout exceeded
    Timeout,
    /// Invalid input/arguments
    InvalidInput,
    /// External service unavailable
    ServiceUnavailable,
    /// Configuration error
    Configuration,
    /// Resource exhausted (disk, memory, etc.)
    ResourceExhausted,
    /// Unknown error
    Unknown,
}

impl FailureCategory {
    /// Get display name
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Permission => "Permission Denied",
            Self::Authentication => "Authentication Failed",
            Self::Network => "Network Error",
            Self::RateLimit => "Rate Limit Exceeded",
            Self::NotFound => "Resource Not Found",
            Self::Timeout => "Request Timeout",
            Self::InvalidInput => "Invalid Input",
            Self::ServiceUnavailable => "Service Unavailable",
            Self::Configuration => "Configuration Error",
            Self::ResourceExhausted => "Resource Exhausted",
            Self::Unknown => "Unknown Error",
        }
    }

    /// Get common causes for this failure type
    #[must_use]
    pub fn common_causes(&self) -> Vec<&'static str> {
        match self {
            Self::Permission => vec![
                "Insufficient file system permissions",
                "Running as wrong user",
                "Sandboxed environment restrictions",
                "SELinux/AppArmor policies",
            ],
            Self::Authentication => vec![
                "API key expired or invalid",
                "Token needs refresh",
                "Missing authentication headers",
                "Wrong credentials",
            ],
            Self::Network => vec![
                "No internet connection",
                "DNS resolution failure",
                "Firewall blocking connection",
                "Proxy configuration issue",
            ],
            Self::RateLimit => vec![
                "Too many requests in short period",
                "Daily/hourly quota exceeded",
                "Concurrent request limit hit",
                "Account tier limits",
            ],
            Self::NotFound => vec![
                "File or directory doesn't exist",
                "Typo in path or URL",
                "Resource was deleted",
                "Case sensitivity issue",
            ],
            Self::Timeout => vec![
                "Operation taking too long",
                "Server not responding",
                "Network latency too high",
                "Insufficient timeout setting",
            ],
            Self::InvalidInput => vec![
                "Missing required parameters",
                "Wrong data type",
                "Value out of range",
                "Malformed JSON/format",
            ],
            Self::ServiceUnavailable => vec![
                "External service is down",
                "Maintenance window",
                "Region outage",
                "Service deprecation",
            ],
            Self::Configuration => vec![
                "Missing environment variable",
                "Invalid config file syntax",
                "Incompatible settings",
                "Wrong config path",
            ],
            Self::ResourceExhausted => vec![
                "Disk space full",
                "Memory limit reached",
                "Too many open files",
                "CPU quota exceeded",
            ],
            Self::Unknown => vec!["Unexpected error occurred", "Check logs for details"],
        }
    }
}
