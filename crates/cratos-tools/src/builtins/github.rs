//! GitHub API tool - GitHub REST API operations

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::time::Instant;
use tracing::{debug, warn};

/// Maximum length for owner/repo names
const MAX_NAME_LENGTH: usize = 100;

/// Validate and sanitize a GitHub owner or repo name
fn validate_name(name: &str, field: &str) -> Result<String> {
    // Check length
    if name.is_empty() {
        return Err(Error::InvalidInput(format!("{} cannot be empty", field)));
    }

    if name.len() > MAX_NAME_LENGTH {
        return Err(Error::InvalidInput(format!(
            "{} exceeds maximum length of {} characters",
            field, MAX_NAME_LENGTH
        )));
    }

    // GitHub only allows alphanumeric, hyphens, and underscores
    // Also dots for repo names in some cases
    let is_valid = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');

    if !is_valid {
        warn!(name = %name, field = %field, "Invalid GitHub name with special characters");
        return Err(Error::InvalidInput(format!(
            "{} contains invalid characters. Only alphanumeric, hyphens, underscores, and dots are allowed.",
            field
        )));
    }

    // Prevent path traversal
    if name.contains("..") {
        return Err(Error::InvalidInput(format!(
            "{} cannot contain '..'",
            field
        )));
    }

    Ok(name.to_string())
}

/// Validate state parameter (open, closed, all)
fn validate_state(state: &str) -> Result<&str> {
    match state {
        "open" | "closed" | "all" => Ok(state),
        _ => {
            warn!(state = %state, "Invalid state parameter");
            Err(Error::InvalidInput(format!(
                "Invalid state '{}'. Must be one of: open, closed, all",
                state
            )))
        }
    }
}

/// Tool for GitHub API operations
pub struct GitHubApiTool {
    definition: ToolDefinition,
    client: reqwest::Client,
}

impl GitHubApiTool {
    /// Create a new GitHub API tool
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let definition = ToolDefinition::new(
            "github_api",
            "Interact with GitHub REST API. Actions: get_repo, list_issues, get_issue, create_issue, \
             list_prs, get_pr, create_pr. Requires GITHUB_TOKEN env var for authentication. \
             Specify owner and repo for all actions. Returns JSON response from GitHub API. \
             Example: {\"action\": \"list_issues\", \"owner\": \"user\", \"repo\": \"project\"} \
             or {\"action\": \"create_issue\", \"owner\": \"user\", \"repo\": \"project\", \"title\": \"Bug report\", \"body\": \"Details...\"}"
        )
            .with_category(ToolCategory::Http)
            .with_risk_level(RiskLevel::Medium)
            .with_parameters(build_parameters_schema());

        Self { definition, client }
    }
}

fn build_parameters_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["get_repo", "list_issues", "get_issue", "create_issue", "list_prs", "get_pr", "create_pr"],
                "description": "GitHub API action to perform"
            },
            "owner": {
                "type": "string",
                "description": "Repository owner"
            },
            "repo": {
                "type": "string",
                "description": "Repository name"
            },
            "number": {
                "type": "integer",
                "description": "Issue or PR number (for get_issue, get_pr)"
            },
            "title": {
                "type": "string",
                "description": "Title (for create_issue, create_pr)"
            },
            "body": {
                "type": "string",
                "description": "Body content (for create_issue, create_pr)"
            },
            "head": {
                "type": "string",
                "description": "Head branch (for create_pr)"
            },
            "base": {
                "type": "string",
                "description": "Base branch (for create_pr)",
                "default": "main"
            },
            "state": {
                "type": "string",
                "enum": ["open", "closed", "all"],
                "description": "Filter by state (for list_issues, list_prs)",
                "default": "open"
            },
            "token": {
                "type": "string",
                "description": "GitHub personal access token (or use GITHUB_TOKEN env var)"
            }
        },
        "required": ["action", "owner", "repo"]
    })
}

impl GitHubApiTool {
    fn get_token(&self, input: &serde_json::Value) -> Result<String> {
        input
            .get("token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("GITHUB_TOKEN").ok())
            .ok_or_else(|| {
                Error::InvalidInput(
                    "GitHub token required (provide 'token' or set GITHUB_TOKEN)".to_string(),
                )
            })
    }
}

impl Default for GitHubApiTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GitHubApiTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'action' parameter".to_string()))?;

        let owner_raw = input
            .get("owner")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'owner' parameter".to_string()))?;

        let repo_raw = input
            .get("repo")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'repo' parameter".to_string()))?;

        // SECURITY: Validate owner and repo names
        let owner = validate_name(owner_raw, "owner")?;
        let repo = validate_name(repo_raw, "repo")?;

        let token = self.get_token(&input)?;

        // Don't log the token
        debug!(action = %action, owner = %owner, repo = %repo, "GitHub API call");

        // Use URL-encoded values in the URL (though validation above should prevent issues)
        let base_url = format!(
            "https://api.github.com/repos/{}/{}",
            urlencoding::encode(&owner),
            urlencoding::encode(&repo)
        );

        let (method, url, body): (&str, String, Option<serde_json::Value>) = match action {
            "get_repo" => ("GET", base_url.clone(), None),

            "list_issues" => {
                let state_raw = input
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("open");
                let state = validate_state(state_raw)?;
                ("GET", format!("{}/issues?state={}", base_url, state), None)
            }

            "get_issue" => {
                let number = input
                    .get("number")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'number' for get_issue".to_string())
                    })?;
                ("GET", format!("{}/issues/{}", base_url, number), None)
            }

            "create_issue" => {
                let title = input.get("title").and_then(|v| v.as_str()).ok_or_else(|| {
                    Error::InvalidInput("Missing 'title' for create_issue".to_string())
                })?;
                let body_content = input.get("body").and_then(|v| v.as_str()).unwrap_or("");
                (
                    "POST",
                    format!("{}/issues", base_url),
                    Some(serde_json::json!({
                        "title": title,
                        "body": body_content
                    })),
                )
            }

            "list_prs" => {
                let state_raw = input
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("open");
                let state = validate_state(state_raw)?;
                ("GET", format!("{}/pulls?state={}", base_url, state), None)
            }

            "get_pr" => {
                let number = input
                    .get("number")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        Error::InvalidInput("Missing 'number' for get_pr".to_string())
                    })?;
                ("GET", format!("{}/pulls/{}", base_url, number), None)
            }

            "create_pr" => {
                let title = input.get("title").and_then(|v| v.as_str()).ok_or_else(|| {
                    Error::InvalidInput("Missing 'title' for create_pr".to_string())
                })?;
                let head = input.get("head").and_then(|v| v.as_str()).ok_or_else(|| {
                    Error::InvalidInput("Missing 'head' for create_pr".to_string())
                })?;
                let base = input.get("base").and_then(|v| v.as_str()).unwrap_or("main");
                let body_content = input.get("body").and_then(|v| v.as_str()).unwrap_or("");
                (
                    "POST",
                    format!("{}/pulls", base_url),
                    Some(serde_json::json!({
                        "title": title,
                        "head": head,
                        "base": base,
                        "body": body_content
                    })),
                )
            }

            _ => {
                return Err(Error::InvalidInput(format!("Unknown action: {}", action)));
            }
        };

        let mut request = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            _ => return Err(Error::InvalidInput("Invalid HTTP method".to_string())),
        };

        request = request
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "cratos-tools/0.1.0")
            .header("X-GitHub-Api-Version", "2022-11-28");

        if let Some(body_json) = body {
            request = request.json(&body_json);
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Network(format!("GitHub API request failed: {}", e)))?;

        let status = response.status().as_u16();
        let response_body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::Network(format!("Failed to parse GitHub response: {}", e)))?;

        let duration = start.elapsed().as_millis() as u64;

        if (200..300).contains(&status) {
            Ok(ToolResult::success(
                serde_json::json!({
                    "action": action,
                    "status": status,
                    "data": response_body
                }),
                duration,
            ))
        } else {
            let error_message = response_body
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            Ok(ToolResult::failure(
                format!("GitHub API error ({}): {}", status, error_message),
                duration,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Tool;

    #[test]
    fn test_github_api_definition() {
        let tool = GitHubApiTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "github_api");
        assert_eq!(def.risk_level, RiskLevel::Medium);
        assert_eq!(def.category, ToolCategory::Http);
    }

    #[tokio::test]
    async fn test_github_api_missing_action() {
        let tool = GitHubApiTool::new();
        let result = tool
            .execute(serde_json::json!({
                "owner": "test",
                "repo": "test"
            }))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_name() {
        // Valid names
        assert!(validate_name("rust-lang", "owner").is_ok());
        assert!(validate_name("cratos_project", "repo").is_ok());
        assert!(validate_name("my.repo", "repo").is_ok());

        // Invalid names - special characters
        assert!(validate_name("test/repo", "repo").is_err());
        assert!(validate_name("test;rm", "owner").is_err());
        assert!(validate_name("test&echo", "owner").is_err());

        // Invalid names - path traversal
        assert!(validate_name("...", "repo").is_err());
        assert!(validate_name("test/../etc", "repo").is_err());

        // Invalid names - empty or too long
        assert!(validate_name("", "owner").is_err());
        let long_name = "a".repeat(200);
        assert!(validate_name(&long_name, "owner").is_err());
    }

    #[test]
    fn test_validate_state() {
        assert!(validate_state("open").is_ok());
        assert!(validate_state("closed").is_ok());
        assert!(validate_state("all").is_ok());

        assert!(validate_state("OPEN").is_err());
        assert!(validate_state("invalid").is_err());
        assert!(validate_state("open;rm").is_err());
    }

    #[tokio::test]
    async fn test_github_api_blocks_injection() {
        let tool = GitHubApiTool::new();

        // Should block injection attempts in owner
        let result = tool
            .execute(serde_json::json!({
                "action": "get_repo",
                "owner": "test/../../../etc",
                "repo": "repo",
                "token": "test"
            }))
            .await;
        assert!(result.is_err());

        // Should block special characters
        let result = tool
            .execute(serde_json::json!({
                "action": "list_issues",
                "owner": "test",
                "repo": "repo;rm -rf /",
                "token": "test"
            }))
            .await;
        assert!(result.is_err());
    }
}
