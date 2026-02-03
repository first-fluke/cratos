//! Tool Doctor - Diagnostic and troubleshooting system
//!
//! This module provides self-diagnosis capabilities for tool failures,
//! generating cause analysis and resolution checklists.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, instrument, warn};

// ============================================================================
// Constants
// ============================================================================

/// Base likelihood percentage for cause ranking
const BASE_LIKELIHOOD: usize = 100;
/// Decrease in likelihood per position in the cause list
const LIKELIHOOD_DECREASE_PER_POSITION: usize = 15;
/// Maximum decrease cap for likelihood
const MAX_LIKELIHOOD_DECREASE: usize = 60;
/// Boost to likelihood when error contains related keyword
const KEYWORD_MATCH_BOOST: usize = 15;
/// Maximum likelihood percentage cap
const MAX_LIKELIHOOD: usize = 95;

// ============================================================================
// Failure Types
// ============================================================================

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

// ============================================================================
// Diagnostic Result
// ============================================================================

/// A diagnosis result from the tool doctor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnosis {
    /// Tool that failed
    pub tool_name: String,
    /// Original error message
    pub error_message: String,
    /// Detected failure category
    pub category: FailureCategory,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Probable causes ranked by likelihood
    pub probable_causes: Vec<ProbableCause>,
    /// Resolution checklist
    pub checklist: Vec<ChecklistItem>,
    /// Alternative approaches if this tool can't work
    pub alternatives: Vec<Alternative>,
}

/// A probable cause with likelihood
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbableCause {
    /// Description of the cause
    pub description: String,
    /// Likelihood percentage (0-100)
    pub likelihood: u8,
    /// How to verify this is the cause
    pub verification: String,
}

/// A checklist item for resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    /// Step number
    pub step: u8,
    /// Action to take
    pub action: String,
    /// Command or instruction
    pub instruction: String,
    /// Expected result
    pub expected_result: String,
}

/// An alternative approach
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    /// Description of alternative
    pub description: String,
    /// Tool to use instead (if any)
    pub tool_name: Option<String>,
    /// Trade-offs of this approach
    pub tradeoffs: String,
}

// ============================================================================
// Tool Doctor
// ============================================================================

/// Tool Doctor - diagnoses tool failures and provides resolution guidance
#[derive(Debug, Clone)]
pub struct ToolDoctor {
    /// Pattern matchers for error categorization
    patterns: HashMap<FailureCategory, Vec<ErrorPattern>>,
    /// Tool-specific handlers
    tool_handlers: HashMap<String, ToolHandler>,
}

/// Pattern for matching errors
#[derive(Debug, Clone)]
struct ErrorPattern {
    /// Patterns to match in error message (case-insensitive)
    keywords: Vec<&'static str>,
    /// Minimum number of keywords that must match
    min_matches: usize,
    /// Boost to confidence if matched
    confidence_boost: f32,
}

/// Tool-specific diagnostic handler
#[derive(Debug, Clone)]
struct ToolHandler {
    /// Known failure modes for this tool
    known_issues: Vec<KnownIssue>,
}

/// A known issue for a specific tool
#[derive(Debug, Clone)]
struct KnownIssue {
    /// Pattern in error
    pattern: &'static str,
    /// Category
    category: FailureCategory,
    /// Specific fix
    fix: &'static str,
}

impl Default for ToolDoctor {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolDoctor {
    /// Create a new tool doctor with default patterns
    #[must_use]
    pub fn new() -> Self {
        let mut doctor = Self {
            patterns: HashMap::new(),
            tool_handlers: HashMap::new(),
        };

        // Initialize error patterns
        doctor.init_patterns();
        // Initialize tool-specific handlers
        doctor.init_tool_handlers();

        doctor
    }

    fn init_patterns(&mut self) {
        // Permission patterns
        self.patterns.insert(
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
        self.patterns.insert(
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
        self.patterns.insert(
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
        self.patterns.insert(
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
        self.patterns.insert(
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
        self.patterns.insert(
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
        self.patterns.insert(
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
        self.patterns.insert(
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
        self.patterns.insert(
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
        self.patterns.insert(
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
    }

    fn init_tool_handlers(&mut self) {
        // HTTP tool issues
        self.tool_handlers.insert(
            "http_get".to_string(),
            ToolHandler {
                known_issues: vec![
                    KnownIssue {
                        pattern: "certificate",
                        category: FailureCategory::Configuration,
                        fix: "Check SSL certificate validity or disable verification for testing",
                    },
                    KnownIssue {
                        pattern: "redirect",
                        category: FailureCategory::InvalidInput,
                        fix: "The URL redirects. Enable redirect following or use the final URL",
                    },
                ],
            },
        );

        // File tool issues
        self.tool_handlers.insert(
            "file_read".to_string(),
            ToolHandler {
                known_issues: vec![
                    KnownIssue {
                        pattern: "binary",
                        category: FailureCategory::InvalidInput,
                        fix: "File appears to be binary. Use appropriate binary reading mode",
                    },
                    KnownIssue {
                        pattern: "encoding",
                        category: FailureCategory::InvalidInput,
                        fix: "File encoding issue. Specify encoding or use UTF-8",
                    },
                ],
            },
        );

        // Git tool issues
        self.tool_handlers.insert(
            "git_clone".to_string(),
            ToolHandler {
                known_issues: vec![
                    KnownIssue {
                        pattern: "already exists",
                        category: FailureCategory::InvalidInput,
                        fix: "Directory already exists. Remove it or use a different path",
                    },
                    KnownIssue {
                        pattern: "private",
                        category: FailureCategory::Authentication,
                        fix: "Repository is private. Configure Git credentials or SSH key",
                    },
                ],
            },
        );

        // GitHub tool issues
        self.tool_handlers.insert(
            "github_create_pr".to_string(),
            ToolHandler {
                known_issues: vec![
                    KnownIssue {
                        pattern: "no commits",
                        category: FailureCategory::InvalidInput,
                        fix: "Branch has no new commits. Push changes before creating PR",
                    },
                    KnownIssue {
                        pattern: "draft",
                        category: FailureCategory::Permission,
                        fix: "Draft PRs may require specific repository settings",
                    },
                ],
            },
        );

        // Exec tool issues
        self.tool_handlers.insert(
            "exec".to_string(),
            ToolHandler {
                known_issues: vec![
                    KnownIssue {
                        pattern: "not found",
                        category: FailureCategory::NotFound,
                        fix: "Command not found. Check PATH or install the required tool",
                    },
                    KnownIssue {
                        pattern: "allowlist",
                        category: FailureCategory::Permission,
                        fix: "Command not in allowlist. Add it to allowed commands or use alternative",
                    },
                ],
            },
        );
    }

    /// Diagnose a tool failure
    #[instrument(skip(self))]
    pub fn diagnose(&self, tool_name: &str, error_message: &str) -> Diagnosis {
        let error_lower = error_message.to_lowercase();

        // Check tool-specific issues first
        let tool_match = self.check_tool_specific(tool_name, &error_lower);

        // Categorize the error
        let (category, confidence) = if let Some((cat, conf)) = tool_match {
            (cat, conf)
        } else {
            self.categorize_error(&error_lower)
        };

        debug!(
            tool = tool_name,
            category = ?category,
            confidence = confidence,
            "Diagnosed tool failure"
        );

        // Build probable causes
        let probable_causes = self.build_probable_causes(category, &error_lower);

        // Build resolution checklist
        let checklist = self.build_checklist(tool_name, category, &error_lower);

        // Find alternatives
        let alternatives = self.find_alternatives(tool_name, category);

        Diagnosis {
            tool_name: tool_name.to_string(),
            error_message: error_message.to_string(),
            category,
            confidence,
            probable_causes,
            checklist,
            alternatives,
        }
    }

    fn check_tool_specific(
        &self,
        tool_name: &str,
        error_lower: &str,
    ) -> Option<(FailureCategory, f32)> {
        if let Some(handler) = self.tool_handlers.get(tool_name) {
            for issue in &handler.known_issues {
                if error_lower.contains(issue.pattern) {
                    return Some((issue.category, 0.95));
                }
            }
        }
        None
    }

    fn categorize_error(&self, error_lower: &str) -> (FailureCategory, f32) {
        let mut best_category = FailureCategory::Unknown;
        let mut best_confidence = 0.0f32;

        for (category, patterns) in &self.patterns {
            for pattern in patterns {
                let matches = pattern
                    .keywords
                    .iter()
                    .filter(|kw| error_lower.contains(*kw))
                    .count();

                if matches >= pattern.min_matches {
                    // Use the boost directly when minimum matches are met
                    // Extra matches increase confidence slightly
                    let extra_boost = if matches > pattern.min_matches {
                        0.05 * (matches - pattern.min_matches) as f32
                    } else {
                        0.0
                    };
                    let confidence = (pattern.confidence_boost + extra_boost).min(0.99);
                    if confidence > best_confidence {
                        best_confidence = confidence;
                        best_category = *category;
                    }
                }
            }
        }

        if best_confidence < 0.3 {
            best_category = FailureCategory::Unknown;
            best_confidence = 0.5; // Default confidence for unknown
        }

        (best_category, best_confidence)
    }

    fn build_probable_causes(
        &self,
        category: FailureCategory,
        error_lower: &str,
    ) -> Vec<ProbableCause> {
        let causes = category.common_causes();
        let mut probable = Vec::with_capacity(causes.len());

        for (i, cause) in causes.iter().enumerate() {
            // Simple heuristic: first causes are more likely
            let base_likelihood =
                BASE_LIKELIHOOD - (i * LIKELIHOOD_DECREASE_PER_POSITION).min(MAX_LIKELIHOOD_DECREASE);

            // Boost if error message contains related keywords
            let boost = if error_lower
                .contains(cause.to_lowercase().split_whitespace().next().unwrap_or(""))
            {
                KEYWORD_MATCH_BOOST
            } else {
                0
            };

            let likelihood = (base_likelihood + boost).min(MAX_LIKELIHOOD) as u8;

            probable.push(ProbableCause {
                description: (*cause).to_string(),
                likelihood,
                verification: self.get_verification(category, i),
            });
        }

        // Sort by likelihood
        probable.sort_by(|a, b| b.likelihood.cmp(&a.likelihood));
        probable
    }

    fn get_verification(&self, category: FailureCategory, index: usize) -> String {
        match (category, index) {
            (FailureCategory::Permission, 0) => "Run `ls -la <path>` to check permissions",
            (FailureCategory::Permission, 1) => "Run `whoami` to check current user",
            (FailureCategory::Authentication, 0) => {
                "Check API key expiration in provider dashboard"
            }
            (FailureCategory::Authentication, 1) => "Try refreshing the token manually",
            (FailureCategory::Network, 0) => "Run `ping 8.8.8.8` to check connectivity",
            (FailureCategory::Network, 1) => "Run `nslookup <domain>` to test DNS",
            (FailureCategory::RateLimit, 0) => "Check API usage in provider dashboard",
            (FailureCategory::NotFound, 0) => "Run `ls -la <path>` to verify existence",
            (FailureCategory::Timeout, 0) => "Try with a longer timeout value",
            (FailureCategory::Configuration, 0) => "Run `env | grep <VAR>` to check variables",
            _ => "Check logs for more details",
        }
        .to_string()
    }

    fn build_checklist(
        &self,
        tool_name: &str,
        category: FailureCategory,
        error_lower: &str,
    ) -> Vec<ChecklistItem> {
        let mut items = Vec::new();

        // Category-specific items
        match category {
            FailureCategory::Permission => {
                items.push(ChecklistItem {
                    step: 1,
                    action: "Check file/directory permissions".to_string(),
                    instruction: "ls -la <target_path>".to_string(),
                    expected_result: "User should have read/write permissions".to_string(),
                });
                items.push(ChecklistItem {
                    step: 2,
                    action: "Fix permissions if needed".to_string(),
                    instruction: "chmod 755 <target_path>".to_string(),
                    expected_result: "Permissions updated".to_string(),
                });
            }
            FailureCategory::Authentication => {
                items.push(ChecklistItem {
                    step: 1,
                    action: "Verify API key is set".to_string(),
                    instruction: "echo $<API_KEY_VAR>".to_string(),
                    expected_result: "API key should be visible".to_string(),
                });
                items.push(ChecklistItem {
                    step: 2,
                    action: "Test API key validity".to_string(),
                    instruction: "Make a simple API call to verify".to_string(),
                    expected_result: "Successful response".to_string(),
                });
                items.push(ChecklistItem {
                    step: 3,
                    action: "Regenerate key if invalid".to_string(),
                    instruction: "Visit provider dashboard to create new key".to_string(),
                    expected_result: "New valid API key".to_string(),
                });
            }
            FailureCategory::Network => {
                items.push(ChecklistItem {
                    step: 1,
                    action: "Check internet connectivity".to_string(),
                    instruction: "ping -c 3 8.8.8.8".to_string(),
                    expected_result: "Packets transmitted successfully".to_string(),
                });
                items.push(ChecklistItem {
                    step: 2,
                    action: "Test DNS resolution".to_string(),
                    instruction: "nslookup <target_domain>".to_string(),
                    expected_result: "Domain resolves to IP".to_string(),
                });
                items.push(ChecklistItem {
                    step: 3,
                    action: "Check firewall/proxy settings".to_string(),
                    instruction: "Review network configuration".to_string(),
                    expected_result: "No blocking rules".to_string(),
                });
            }
            FailureCategory::RateLimit => {
                items.push(ChecklistItem {
                    step: 1,
                    action: "Wait before retrying".to_string(),
                    instruction: "Wait 60 seconds, then retry".to_string(),
                    expected_result: "Rate limit reset".to_string(),
                });
                items.push(ChecklistItem {
                    step: 2,
                    action: "Check usage quotas".to_string(),
                    instruction: "Review API usage in provider dashboard".to_string(),
                    expected_result: "Understand current usage".to_string(),
                });
                items.push(ChecklistItem {
                    step: 3,
                    action: "Consider upgrading plan".to_string(),
                    instruction: "Review pricing tiers for higher limits".to_string(),
                    expected_result: "Plan with sufficient quota".to_string(),
                });
            }
            FailureCategory::NotFound => {
                items.push(ChecklistItem {
                    step: 1,
                    action: "Verify path exists".to_string(),
                    instruction: "ls -la <path>".to_string(),
                    expected_result: "Path should exist".to_string(),
                });
                items.push(ChecklistItem {
                    step: 2,
                    action: "Check for typos".to_string(),
                    instruction: "Double-check the path spelling".to_string(),
                    expected_result: "Correct path identified".to_string(),
                });
                items.push(ChecklistItem {
                    step: 3,
                    action: "Check case sensitivity".to_string(),
                    instruction: "Use tab completion to verify name".to_string(),
                    expected_result: "Correct case used".to_string(),
                });
            }
            FailureCategory::Timeout => {
                items.push(ChecklistItem {
                    step: 1,
                    action: "Increase timeout".to_string(),
                    instruction: "Set timeout to a higher value (e.g., 60s)".to_string(),
                    expected_result: "Operation completes within new timeout".to_string(),
                });
                items.push(ChecklistItem {
                    step: 2,
                    action: "Check server status".to_string(),
                    instruction: "Verify the target service is running".to_string(),
                    expected_result: "Service is healthy".to_string(),
                });
            }
            _ => {
                items.push(ChecklistItem {
                    step: 1,
                    action: "Review error message".to_string(),
                    instruction: "Read the full error for specific details".to_string(),
                    expected_result: "Identify specific issue".to_string(),
                });
                items.push(ChecklistItem {
                    step: 2,
                    action: "Check logs".to_string(),
                    instruction: "Review application logs for more context".to_string(),
                    expected_result: "Additional error details".to_string(),
                });
            }
        }

        // Tool-specific additions
        if let Some(handler) = self.tool_handlers.get(tool_name) {
            for issue in &handler.known_issues {
                if error_lower.contains(issue.pattern) {
                    items.push(ChecklistItem {
                        step: items.len() as u8 + 1,
                        action: format!("Tool-specific fix for '{}'", issue.pattern),
                        instruction: issue.fix.to_string(),
                        expected_result: "Issue resolved".to_string(),
                    });
                }
            }
        }

        items
    }

    fn find_alternatives(&self, tool_name: &str, category: FailureCategory) -> Vec<Alternative> {
        let mut alternatives = Vec::new();

        match tool_name {
            "http_get" => {
                alternatives.push(Alternative {
                    description: "Use curl via exec tool".to_string(),
                    tool_name: Some("exec".to_string()),
                    tradeoffs: "Requires curl to be installed and in allowlist".to_string(),
                });
            }
            "file_read" => {
                if category == FailureCategory::Permission {
                    alternatives.push(Alternative {
                        description: "Read file with elevated permissions".to_string(),
                        tool_name: Some("exec".to_string()),
                        tradeoffs: "Requires sudo access".to_string(),
                    });
                }
            }
            "git_clone" => {
                alternatives.push(Alternative {
                    description: "Download as ZIP from GitHub".to_string(),
                    tool_name: Some("http_get".to_string()),
                    tradeoffs: "No git history, manual extraction needed".to_string(),
                });
            }
            "github_create_pr" => {
                alternatives.push(Alternative {
                    description: "Create PR manually via GitHub UI".to_string(),
                    tool_name: None,
                    tradeoffs: "Requires manual user action".to_string(),
                });
            }
            _ => {}
        }

        // Generic alternatives based on category
        if category == FailureCategory::RateLimit {
            alternatives.push(Alternative {
                description: "Queue operation for later".to_string(),
                tool_name: None,
                tradeoffs: "Delayed execution".to_string(),
            });
        }

        if category == FailureCategory::ServiceUnavailable {
            alternatives.push(Alternative {
                description: "Use cached data if available".to_string(),
                tool_name: None,
                tradeoffs: "Data may be stale".to_string(),
            });
        }

        alternatives
    }

    /// Format diagnosis as user-friendly text
    #[must_use]
    pub fn format_diagnosis(&self, diagnosis: &Diagnosis) -> String {
        let mut output = String::new();

        output.push_str("🔍 **Tool Doctor Diagnosis**\n\n");
        output.push_str(&format!("**Tool:** `{}`\n", diagnosis.tool_name));
        output.push_str(&format!(
            "**Error Category:** {} ({}% confidence)\n\n",
            diagnosis.category.display_name(),
            (diagnosis.confidence * 100.0) as u8
        ));

        output.push_str("**Probable Causes:**\n");
        for (i, cause) in diagnosis.probable_causes.iter().take(3).enumerate() {
            output.push_str(&format!(
                "{}. {} ({}% likely)\n   → {}\n",
                i + 1,
                cause.description,
                cause.likelihood,
                cause.verification
            ));
        }

        output.push_str("\n**Resolution Checklist:**\n");
        for item in &diagnosis.checklist {
            output.push_str(&format!(
                "☐ Step {}: {}\n   ```\n   {}\n   ```\n   Expected: {}\n",
                item.step, item.action, item.instruction, item.expected_result
            ));
        }

        if !diagnosis.alternatives.is_empty() {
            output.push_str("\n**Alternative Approaches:**\n");
            for alt in &diagnosis.alternatives {
                if let Some(tool) = &alt.tool_name {
                    output.push_str(&format!("• {} (use `{}`)\n", alt.description, tool));
                } else {
                    output.push_str(&format!("• {}\n", alt.description));
                }
                output.push_str(&format!("  Trade-offs: {}\n", alt.tradeoffs));
            }
        }

        output
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnose_permission_error() {
        let doctor = ToolDoctor::new();
        let diagnosis = doctor.diagnose("file_read", "Error: Permission denied: /etc/shadow");

        assert_eq!(diagnosis.category, FailureCategory::Permission);
        assert!(diagnosis.confidence > 0.8);
        assert!(!diagnosis.probable_causes.is_empty());
        assert!(!diagnosis.checklist.is_empty());
    }

    #[test]
    fn test_diagnose_authentication_error() {
        let doctor = ToolDoctor::new();
        let diagnosis = doctor.diagnose("http_get", "401 Unauthorized: Invalid API key");

        assert_eq!(diagnosis.category, FailureCategory::Authentication);
        assert!(diagnosis.confidence > 0.8);
    }

    #[test]
    fn test_diagnose_network_error() {
        let doctor = ToolDoctor::new();
        let diagnosis = doctor.diagnose("http_get", "Connection refused: server is not responding");

        assert_eq!(diagnosis.category, FailureCategory::Network);
        assert!(diagnosis.confidence > 0.8);
    }

    #[test]
    fn test_diagnose_rate_limit_error() {
        let doctor = ToolDoctor::new();
        let diagnosis = doctor.diagnose(
            "github_create_pr",
            "429 Too Many Requests: rate limit exceeded",
        );

        assert_eq!(diagnosis.category, FailureCategory::RateLimit);
        assert!(diagnosis.confidence > 0.9);
    }

    #[test]
    fn test_diagnose_not_found_error() {
        let doctor = ToolDoctor::new();
        let diagnosis = doctor.diagnose(
            "file_read",
            "Error: No such file or directory: /path/to/missing",
        );

        assert_eq!(diagnosis.category, FailureCategory::NotFound);
        assert!(diagnosis.confidence > 0.8);
    }

    #[test]
    fn test_diagnose_timeout_error() {
        let doctor = ToolDoctor::new();
        let diagnosis =
            doctor.diagnose("http_get", "Request timeout: operation timed out after 30s");

        assert_eq!(diagnosis.category, FailureCategory::Timeout);
        assert!(diagnosis.confidence > 0.8);
    }

    #[test]
    fn test_diagnose_unknown_error() {
        let doctor = ToolDoctor::new();
        let diagnosis = doctor.diagnose("some_tool", "Something went wrong");

        assert_eq!(diagnosis.category, FailureCategory::Unknown);
    }

    #[test]
    fn test_tool_specific_diagnosis() {
        let doctor = ToolDoctor::new();

        // Git clone with directory already exists
        let diagnosis = doctor.diagnose("git_clone", "Error: directory already exists");
        assert_eq!(diagnosis.category, FailureCategory::InvalidInput);
        assert!(diagnosis.confidence > 0.9);
    }

    #[test]
    fn test_checklist_has_items() {
        let doctor = ToolDoctor::new();
        let diagnosis = doctor.diagnose("file_read", "Permission denied");

        assert!(diagnosis.checklist.len() >= 2);
        assert_eq!(diagnosis.checklist[0].step, 1);
    }

    #[test]
    fn test_alternatives_for_http() {
        let doctor = ToolDoctor::new();
        let diagnosis = doctor.diagnose("http_get", "Connection refused");

        assert!(!diagnosis.alternatives.is_empty());
        assert!(diagnosis
            .alternatives
            .iter()
            .any(|a| a.tool_name.as_deref() == Some("exec")));
    }

    #[test]
    fn test_format_diagnosis() {
        let doctor = ToolDoctor::new();
        let diagnosis = doctor.diagnose("file_read", "Permission denied: /etc/shadow");

        let formatted = doctor.format_diagnosis(&diagnosis);

        assert!(formatted.contains("Tool Doctor Diagnosis"));
        assert!(formatted.contains("Permission Denied"));
        assert!(formatted.contains("Probable Causes"));
        assert!(formatted.contains("Resolution Checklist"));
    }

    #[test]
    fn test_failure_category_display() {
        assert_eq!(
            FailureCategory::Permission.display_name(),
            "Permission Denied"
        );
        assert_eq!(
            FailureCategory::Authentication.display_name(),
            "Authentication Failed"
        );
        assert_eq!(FailureCategory::Network.display_name(), "Network Error");
        assert_eq!(
            FailureCategory::RateLimit.display_name(),
            "Rate Limit Exceeded"
        );
    }

    #[test]
    fn test_failure_category_common_causes() {
        let causes = FailureCategory::Permission.common_causes();
        assert!(!causes.is_empty());
        assert!(causes.iter().any(|c| c.contains("permission")));
    }
}
