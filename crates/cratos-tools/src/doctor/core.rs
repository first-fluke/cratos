//! Tool Doctor - Core implementation

use super::category::FailureCategory;
use super::formatter;
use super::handlers::{init_tool_handlers, ToolHandler};
use super::patterns::{init_patterns, ErrorPattern};
use super::types::{Alternative, ChecklistItem, Diagnosis, ProbableCause};
use std::collections::HashMap;
use tracing::{debug, instrument};

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

/// Tool Doctor - diagnoses tool failures and provides resolution guidance
#[derive(Debug, Clone)]
pub struct ToolDoctor {
    /// Pattern matchers for error categorization
    patterns: HashMap<FailureCategory, Vec<ErrorPattern>>,
    /// Tool-specific handlers
    tool_handlers: HashMap<String, ToolHandler>,
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
        Self {
            patterns: init_patterns(),
            tool_handlers: init_tool_handlers(),
        }
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
            let base_likelihood = BASE_LIKELIHOOD
                - (i * LIKELIHOOD_DECREASE_PER_POSITION).min(MAX_LIKELIHOOD_DECREASE);

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
        formatter::format_diagnosis(diagnosis)
    }
}
