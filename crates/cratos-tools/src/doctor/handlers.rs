//! Tool-specific diagnostic handlers

use super::category::FailureCategory;
use std::collections::HashMap;

/// Tool-specific diagnostic handler
#[derive(Debug, Clone)]
pub struct ToolHandler {
    /// Known failure modes for this tool
    pub known_issues: Vec<KnownIssue>,
}

/// A known issue for a specific tool
#[derive(Debug, Clone)]
pub struct KnownIssue {
    /// Pattern in error
    pub pattern: &'static str,
    /// Category
    pub category: FailureCategory,
    /// Specific fix
    pub fix: &'static str,
}

/// Initialize tool-specific handlers
pub fn init_tool_handlers() -> HashMap<String, ToolHandler> {
    let mut handlers = HashMap::new();

    // HTTP tool issues
    handlers.insert(
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
    handlers.insert(
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
    handlers.insert(
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
    handlers.insert(
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
    handlers.insert(
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

    handlers
}
