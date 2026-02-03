//! Git tools - Git repository operations

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tracing::debug;

// ============================================================================
// Security Constants
// ============================================================================

/// Dangerous git flags that should be blocked
#[allow(dead_code)] // Used in tests and for documentation
const BLOCKED_FLAGS: &[&str] = &[
    "--force",
    "-f",
    "--force-with-lease",
    "--no-verify",
    "-n",
    "--hard",
    "--delete",
    "-D",
    "--force-delete",
    "--mirror",
    "--prune",
    "--all", // when used with push
];

/// Check if an argument contains blocked flags
#[allow(dead_code)] // Used in tests and for future validation
fn contains_blocked_flag(arg: &str) -> bool {
    BLOCKED_FLAGS
        .iter()
        .any(|flag| arg == *flag || arg.starts_with(&format!("{}=", flag)))
}

/// Validate branch name for security (prevent command injection)
fn is_valid_branch_name(name: &str) -> bool {
    // Git branch names have specific restrictions
    // Reject anything that looks like it could be command injection
    if name.is_empty() || name.len() > 255 {
        return false;
    }

    // Must not start with - (could be interpreted as flag)
    if name.starts_with('-') {
        return false;
    }

    // Must not contain dangerous characters
    let dangerous_chars = ['`', '$', '|', ';', '&', '>', '<', '\n', '\r', '\0'];
    if name.chars().any(|c| dangerous_chars.contains(&c)) {
        return false;
    }

    // Must not contain .. (path traversal)
    if name.contains("..") {
        return false;
    }

    true
}

// ============================================================================
// Git Status Tool
// ============================================================================

/// Tool for getting git repository status
pub struct GitStatusTool {
    definition: ToolDefinition,
}

impl GitStatusTool {
    /// Create a new git status tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new("git_status", "Get the status of a git repository")
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::Low)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "short": {
                        "type": "boolean",
                        "description": "Use short format output",
                        "default": false
                    }
                }
            }));

        Self { definition }
    }
}

impl Default for GitStatusTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitStatusTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let short = input
            .get("short")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        debug!(path = %path, "Getting git status");

        let mut cmd = Command::new("git");
        cmd.arg("status");
        if short {
            cmd.arg("--short");
        }
        cmd.arg("--porcelain=v1");
        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run git status: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            // Parse porcelain output
            let files: Vec<serde_json::Value> = stdout
                .lines()
                .filter(|line| !line.is_empty())
                .map(|line| {
                    let status = &line[0..2];
                    let file = line[3..].to_string();
                    serde_json::json!({
                        "status": status.trim(),
                        "file": file
                    })
                })
                .collect();

            Ok(ToolResult::success(
                serde_json::json!({
                    "clean": files.is_empty(),
                    "files": files,
                    "count": files.len()
                }),
                duration,
            ))
        } else {
            Ok(ToolResult::failure(
                format!("git status failed: {}", stderr),
                duration,
            ))
        }
    }
}

// ============================================================================
// Git Commit Tool
// ============================================================================

/// Tool for creating git commits
pub struct GitCommitTool {
    definition: ToolDefinition,
}

impl GitCommitTool {
    /// Create a new git commit tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new("git_commit", "Create a git commit")
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::Medium)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Commit message"
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "add_all": {
                        "type": "boolean",
                        "description": "Stage all changes before committing",
                        "default": false
                    },
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Specific files to stage before committing"
                    }
                },
                "required": ["message"]
            }));

        Self { definition }
    }
}

impl Default for GitCommitTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitCommitTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let message = input
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'message' parameter".to_string()))?;

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let add_all = input
            .get("add_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let files: Vec<String> = input
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        debug!(path = %path, message = %message, "Creating git commit");

        // Stage files if requested
        if add_all {
            let mut add_cmd = Command::new("git");
            add_cmd.args(["add", "-A"]);
            add_cmd.current_dir(path);
            add_cmd
                .output()
                .await
                .map_err(|e| Error::Execution(format!("Failed to stage all files: {}", e)))?;
        } else if !files.is_empty() {
            let mut add_cmd = Command::new("git");
            add_cmd.arg("add");
            add_cmd.args(&files);
            add_cmd.current_dir(path);
            add_cmd
                .output()
                .await
                .map_err(|e| Error::Execution(format!("Failed to stage files: {}", e)))?;
        }

        // Create commit
        let mut cmd = Command::new("git");
        cmd.args(["commit", "-m", message]);
        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to create commit: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            // Extract commit hash from output
            let commit_hash = stdout
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .map(|s| s.trim_matches(|c| c == '[' || c == ']'))
                .unwrap_or("")
                .to_string();

            Ok(ToolResult::success(
                serde_json::json!({
                    "success": true,
                    "commit": commit_hash,
                    "message": message,
                    "output": stdout
                }),
                duration,
            ))
        } else {
            Ok(ToolResult::failure(
                format!("git commit failed: {}", stderr),
                duration,
            ))
        }
    }
}

// ============================================================================
// Git Branch Tool
// ============================================================================

/// Tool for git branch operations
pub struct GitBranchTool {
    definition: ToolDefinition,
}

impl GitBranchTool {
    /// Create a new git branch tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new("git_branch", "Manage git branches")
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::Medium)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list", "create", "checkout", "delete"],
                        "description": "Branch action to perform",
                        "default": "list"
                    },
                    "name": {
                        "type": "string",
                        "description": "Branch name (for create/checkout/delete)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    }
                }
            }));

        Self { definition }
    }
}

impl Default for GitBranchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitBranchTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let name = input.get("name").and_then(|v| v.as_str());

        debug!(path = %path, action = %action, "Git branch operation");

        let mut cmd = Command::new("git");
        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        match action {
            "list" => {
                cmd.args([
                    "branch",
                    "-a",
                    "--format=%(refname:short) %(objectname:short) %(upstream:short)",
                ]);
            }
            "create" => {
                let branch_name = name.ok_or_else(|| {
                    Error::InvalidInput("Branch name required for create".to_string())
                })?;
                // SECURITY: Validate branch name
                if !is_valid_branch_name(branch_name) {
                    return Err(Error::InvalidInput(format!(
                        "Invalid branch name: {}",
                        branch_name
                    )));
                }
                cmd.args(["branch", branch_name]);
            }
            "checkout" => {
                let branch_name = name.ok_or_else(|| {
                    Error::InvalidInput("Branch name required for checkout".to_string())
                })?;
                // SECURITY: Validate branch name
                if !is_valid_branch_name(branch_name) {
                    return Err(Error::InvalidInput(format!(
                        "Invalid branch name: {}",
                        branch_name
                    )));
                }
                cmd.args(["checkout", branch_name]);
            }
            "delete" => {
                let branch_name = name.ok_or_else(|| {
                    Error::InvalidInput("Branch name required for delete".to_string())
                })?;
                // SECURITY: Validate branch name
                if !is_valid_branch_name(branch_name) {
                    return Err(Error::InvalidInput(format!(
                        "Invalid branch name: {}",
                        branch_name
                    )));
                }
                // SECURITY: Use -d (safe delete) instead of -D (force delete)
                cmd.args(["branch", "-d", branch_name]);
            }
            _ => {
                return Err(Error::InvalidInput(format!("Unknown action: {}", action)));
            }
        }

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run git branch: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            match action {
                "list" => {
                    let branches: Vec<serde_json::Value> = stdout
                        .lines()
                        .filter(|line| !line.is_empty())
                        .map(|line| {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            serde_json::json!({
                                "name": parts.first().unwrap_or(&""),
                                "commit": parts.get(1).unwrap_or(&""),
                                "upstream": parts.get(2).unwrap_or(&"")
                            })
                        })
                        .collect();

                    Ok(ToolResult::success(
                        serde_json::json!({
                            "action": action,
                            "branches": branches,
                            "count": branches.len()
                        }),
                        duration,
                    ))
                }
                _ => Ok(ToolResult::success(
                    serde_json::json!({
                        "action": action,
                        "name": name,
                        "success": true,
                        "output": stdout
                    }),
                    duration,
                )),
            }
        } else {
            Ok(ToolResult::failure(
                format!("git branch {} failed: {}", action, stderr),
                duration,
            ))
        }
    }
}

// ============================================================================
// Git Diff Tool
// ============================================================================

/// Tool for showing git diffs
pub struct GitDiffTool {
    definition: ToolDefinition,
}

impl GitDiffTool {
    /// Create a new git diff tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new("git_diff", "Show git diff")
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::Low)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "staged": {
                        "type": "boolean",
                        "description": "Show staged changes only",
                        "default": false
                    },
                    "commit": {
                        "type": "string",
                        "description": "Compare with specific commit"
                    },
                    "file": {
                        "type": "string",
                        "description": "Show diff for specific file"
                    },
                    "stat": {
                        "type": "boolean",
                        "description": "Show diffstat only",
                        "default": false
                    }
                }
            }));

        Self { definition }
    }
}

impl Default for GitDiffTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitDiffTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let staged = input
            .get("staged")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let commit = input.get("commit").and_then(|v| v.as_str());
        let file = input.get("file").and_then(|v| v.as_str());

        let stat = input.get("stat").and_then(|v| v.as_bool()).unwrap_or(false);

        debug!(path = %path, "Getting git diff");

        let mut cmd = Command::new("git");
        cmd.arg("diff");

        if staged {
            cmd.arg("--staged");
        }

        if stat {
            cmd.arg("--stat");
        }

        if let Some(c) = commit {
            cmd.arg(c);
        }

        if let Some(f) = file {
            cmd.arg("--");
            cmd.arg(f);
        }

        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run git diff: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            Ok(ToolResult::success(
                serde_json::json!({
                    "diff": stdout,
                    "has_changes": !stdout.is_empty(),
                    "staged": staged
                }),
                duration,
            ))
        } else {
            Ok(ToolResult::failure(
                format!("git diff failed: {}", stderr),
                duration,
            ))
        }
    }
}

// ============================================================================
// Git Push Tool
// ============================================================================

/// Tool for pushing to remote repositories (with security restrictions)
pub struct GitPushTool {
    definition: ToolDefinition,
}

impl GitPushTool {
    /// Create a new git push tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new("git_push", "Push commits to remote repository")
            .with_category(ToolCategory::Utility)
            .with_risk_level(RiskLevel::High)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the git repository (default: current directory)"
                    },
                    "remote": {
                        "type": "string",
                        "description": "Remote name (default: origin)",
                        "default": "origin"
                    },
                    "branch": {
                        "type": "string",
                        "description": "Branch to push (default: current branch)"
                    },
                    "set_upstream": {
                        "type": "boolean",
                        "description": "Set upstream tracking",
                        "default": false
                    }
                }
            }));

        Self { definition }
    }
}

impl Default for GitPushTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitPushTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let remote = input
            .get("remote")
            .and_then(|v| v.as_str())
            .unwrap_or("origin");
        let branch = input.get("branch").and_then(|v| v.as_str());
        let set_upstream = input
            .get("set_upstream")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // SECURITY: Validate remote name
        if !is_valid_branch_name(remote) {
            return Err(Error::InvalidInput(format!(
                "Invalid remote name: {}",
                remote
            )));
        }

        // SECURITY: Validate branch name if provided
        if let Some(b) = branch {
            if !is_valid_branch_name(b) {
                return Err(Error::InvalidInput(format!("Invalid branch name: {}", b)));
            }
        }

        debug!(path = %path, remote = %remote, branch = ?branch, "Git push");

        let mut cmd = Command::new("git");
        cmd.arg("push");

        // SECURITY: Never allow force push flags
        // The tool intentionally does not expose --force, -f, or --force-with-lease

        if set_upstream {
            cmd.arg("-u");
        }

        cmd.arg(remote);

        if let Some(b) = branch {
            cmd.arg(b);
        }

        cmd.current_dir(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Failed to run git push: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let duration = start.elapsed().as_millis() as u64;

        if output.status.success() {
            Ok(ToolResult::success(
                serde_json::json!({
                    "success": true,
                    "remote": remote,
                    "branch": branch,
                    "output": format!("{}{}", stdout, stderr)
                }),
                duration,
            ))
        } else {
            // SECURITY: Sanitize error message (don't leak remote URLs with tokens)
            let sanitized_error = if stderr.contains("http") || stderr.contains("@") {
                "git push failed: authentication or remote error".to_string()
            } else {
                format!("git push failed: {}", stderr)
            };

            Ok(ToolResult::failure(sanitized_error, duration))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Tool;

    #[test]
    fn test_git_status_definition() {
        let tool = GitStatusTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "git_status");
        assert_eq!(def.risk_level, RiskLevel::Low);
        assert_eq!(def.category, ToolCategory::Utility);
    }

    #[test]
    fn test_git_commit_definition() {
        let tool = GitCommitTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "git_commit");
        assert_eq!(def.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_git_branch_definition() {
        let tool = GitBranchTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "git_branch");
        assert_eq!(def.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_git_diff_definition() {
        let tool = GitDiffTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "git_diff");
        assert_eq!(def.risk_level, RiskLevel::Low);
    }

    #[tokio::test]
    async fn test_git_commit_missing_message() {
        let tool = GitCommitTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    // Security tests

    #[test]
    fn test_blocked_flags_detection() {
        assert!(contains_blocked_flag("--force"));
        assert!(contains_blocked_flag("-f"));
        assert!(contains_blocked_flag("--no-verify"));
        assert!(contains_blocked_flag("-D"));
        assert!(contains_blocked_flag("--hard"));

        // These should not be blocked
        assert!(!contains_blocked_flag("main"));
        assert!(!contains_blocked_flag("feature/test"));
        assert!(!contains_blocked_flag("-m"));
    }

    #[test]
    fn test_valid_branch_names() {
        assert!(is_valid_branch_name("main"));
        assert!(is_valid_branch_name("feature/new-feature"));
        assert!(is_valid_branch_name("fix-123"));

        // Invalid names
        assert!(!is_valid_branch_name("-flag"));
        assert!(!is_valid_branch_name("branch;rm -rf /"));
        assert!(!is_valid_branch_name("branch`whoami`"));
        assert!(!is_valid_branch_name("branch$PATH"));
        assert!(!is_valid_branch_name("branch|cat /etc/passwd"));
        assert!(!is_valid_branch_name("branch..traversal"));
        assert!(!is_valid_branch_name(""));
    }

    #[test]
    fn test_git_push_definition() {
        let tool = GitPushTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "git_push");
        assert_eq!(def.risk_level, RiskLevel::High);
    }

    #[tokio::test]
    async fn test_git_branch_rejects_invalid_name() {
        let tool = GitBranchTool::new();

        // Test command injection attempt
        let result = tool
            .execute(serde_json::json!({
                "action": "create",
                "name": "branch;rm -rf /"
            }))
            .await;
        assert!(result.is_err());

        // Test flag injection attempt
        let result = tool
            .execute(serde_json::json!({
                "action": "create",
                "name": "--force"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_git_push_rejects_invalid_remote() {
        let tool = GitPushTool::new();

        let result = tool
            .execute(serde_json::json!({
                "remote": "-f origin"
            }))
            .await;
        assert!(result.is_err());
    }
}
