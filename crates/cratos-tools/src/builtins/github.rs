//! GitHub API tool - GitHub REST API operations

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::time::Instant;
use tracing::debug;

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

        let definition = ToolDefinition::new("github_api", "Interact with GitHub API")
            .with_category(ToolCategory::Http)
            .with_risk_level(RiskLevel::Medium)
            .with_parameters(serde_json::json!({
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
            }));

        Self { definition, client }
    }

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

        let owner = input
            .get("owner")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'owner' parameter".to_string()))?;

        let repo = input
            .get("repo")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'repo' parameter".to_string()))?;

        let token = self.get_token(&input)?;

        debug!(action = %action, owner = %owner, repo = %repo, "GitHub API call");

        let base_url = format!("https://api.github.com/repos/{}/{}", owner, repo);

        let (method, url, body): (&str, String, Option<serde_json::Value>) = match action {
            "get_repo" => ("GET", base_url.clone(), None),

            "list_issues" => {
                let state = input
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("open");
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
                let state = input
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("open");
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
}
