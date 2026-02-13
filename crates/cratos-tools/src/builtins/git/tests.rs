//! Tests for git tools

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
    use security::contains_blocked_flag;

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
    use security::is_valid_branch_name;

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

// Git Clone tests

#[test]
fn test_git_clone_definition() {
    let tool = GitCloneTool::new();
    let def = tool.definition();

    assert_eq!(def.name, "git_clone");
    assert_eq!(def.risk_level, RiskLevel::Medium);
    assert_eq!(def.category, ToolCategory::Utility);
}

#[tokio::test]
async fn test_git_clone_missing_url() {
    let tool = GitCloneTool::new();
    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_git_clone_rejects_invalid_url() {
    let tool = GitCloneTool::new();

    // javascript: protocol
    let result = tool
        .execute(serde_json::json!({"url": "javascript:alert(1)"}))
        .await;
    assert!(result.is_err());

    // Shell injection in URL
    let result = tool
        .execute(serde_json::json!({"url": "https://evil.com/$(whoami)"}))
        .await;
    assert!(result.is_err());

    // No protocol
    let result = tool
        .execute(serde_json::json!({"url": "ftp://example.com/repo.git"}))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_git_clone_rejects_path_traversal() {
    let tool = GitCloneTool::new();

    let result = tool
        .execute(serde_json::json!({
            "url": "https://github.com/user/repo.git",
            "path": "../../etc/passwd"
        }))
        .await;
    assert!(result.is_err());
}

#[test]
fn test_valid_clone_urls() {
    use security::is_valid_clone_url;

    assert!(is_valid_clone_url("https://github.com/user/repo.git"));
    assert!(is_valid_clone_url("git://github.com/user/repo.git"));
    assert!(is_valid_clone_url("ssh://git@github.com/user/repo.git"));
    assert!(is_valid_clone_url("git@github.com:user/repo.git"));

    // Invalid protocols
    assert!(!is_valid_clone_url("javascript:alert(1)"));
    assert!(!is_valid_clone_url("data:text/html,<script>"));
    assert!(!is_valid_clone_url("ftp://example.com/repo"));

    // Shell injection
    assert!(!is_valid_clone_url("https://example.com/$(whoami)"));
    assert!(!is_valid_clone_url("https://example.com/`id`"));
    assert!(!is_valid_clone_url("https://example.com/;rm -rf /"));
}

#[test]
fn test_valid_clone_paths() {
    use security::is_valid_clone_path;

    assert!(is_valid_clone_path("/tmp/myrepo"));
    assert!(is_valid_clone_path("repos/project"));

    // Path traversal
    assert!(!is_valid_clone_path("../../etc"));
    assert!(!is_valid_clone_path("foo/../bar"));

    // Shell injection
    assert!(!is_valid_clone_path("repo;rm -rf /"));
    assert!(!is_valid_clone_path("repo`id`"));

    // Empty / too long
    assert!(!is_valid_clone_path(""));
}

// Git Log tests

#[test]
fn test_git_log_definition() {
    let tool = GitLogTool::new();
    let def = tool.definition();

    assert_eq!(def.name, "git_log");
    assert_eq!(def.risk_level, RiskLevel::Low);
    assert_eq!(def.category, ToolCategory::Utility);
}

#[tokio::test]
async fn test_git_log_rejects_invalid_branch() {
    let tool = GitLogTool::new();

    let result = tool
        .execute(serde_json::json!({
            "branch": "-flag-injection"
        }))
        .await;
    assert!(result.is_err());
}
