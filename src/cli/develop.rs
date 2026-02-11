//! CLI handler for the `/develop` command.
//!
//! Automates the Issue → PR workflow by loading the develop workflow prompt
//! and running it through the orchestrator.

use anyhow::{Context, Result};
use cratos_core::OrchestratorInput;
use tracing::info;

/// Default workflow prompt when `.agent/workflows/develop.md` is not found.
const FALLBACK_WORKFLOW_PROMPT: &str = r#"You are a development workflow agent. Follow these steps:
1. Analyze the GitHub issue
2. Clone the repository if needed (use git_clone tool)
3. Create a feature branch (use git_branch tool)
4. Implement the fix or feature using available tools
5. Commit changes (use git_commit tool)
6. Push and create a PR (use github_api tool with create_pr action)

Use the available tools (exec, git_*, github_api) to complete each step.
Report progress after each step."#;

/// Load the develop workflow prompt from `.agent/workflows/develop.md`.
fn load_workflow_prompt() -> String {
    let workflow_path = std::path::Path::new(".agent/workflows/develop.md");
    if workflow_path.exists() {
        match std::fs::read_to_string(workflow_path) {
            Ok(content) if !content.trim().is_empty() => {
                info!("Loaded develop workflow from {}", workflow_path.display());
                return content;
            }
            Ok(_) => {
                info!("Develop workflow file is empty, using fallback");
            }
            Err(e) => {
                info!("Failed to read develop workflow: {}, using fallback", e);
            }
        }
    }
    FALLBACK_WORKFLOW_PROMPT.to_string()
}

/// Build the user prompt from issue reference and options.
fn build_user_prompt(issue: &str, repo: Option<&str>, dry_run: bool) -> String {
    let mut prompt = format!("Resolve this GitHub issue: {}", issue);
    if let Some(repo_url) = repo {
        prompt.push_str(&format!("\nRepository: {}", repo_url));
    }
    if dry_run {
        prompt.push_str("\n\nDRY RUN: Show the implementation plan without executing any changes. List the files you would modify and the approach you would take.");
    }
    prompt
}

/// Run the develop workflow.
pub async fn run(issue: &str, repo: Option<&str>, dry_run: bool) -> Result<()> {
    println!("Starting /develop workflow...");

    // 1. Load workflow system prompt
    let workflow_prompt = load_workflow_prompt();

    // 2. Load config and build orchestrator
    let config = crate::server::load_config().context("Failed to load configuration")?;
    let orchestrator = crate::server::build_orchestrator_for_cli(&config)
        .await
        .context("Failed to build orchestrator")?;

    // 3. Build user prompt
    let user_prompt = build_user_prompt(issue, repo, dry_run);

    // 4. Create input with system prompt override
    let input = OrchestratorInput::new("cli", "develop", "user", &user_prompt)
        .with_system_prompt_override(workflow_prompt);

    // 5. Execute
    if dry_run {
        println!("Mode: DRY RUN (no changes will be made)");
    }
    println!("Issue: {}", issue);
    if let Some(r) = repo {
        println!("Repository: {}", r);
    }
    println!("---");

    let result = orchestrator.process(input).await
        .map_err(|e| anyhow::anyhow!("Orchestrator execution failed: {}", e))?;

    println!("{}", result.response);

    if !result.tool_calls.is_empty() {
        println!("\n--- Tool Usage Summary ---");
        for tc in &result.tool_calls {
            let status = if tc.success { "OK" } else { "FAIL" };
            println!("  {} [{}] ({}ms)", tc.tool_name, status, tc.duration_ms);
        }
    }

    println!(
        "\nCompleted in {}ms ({} iterations, {} tool calls)",
        result.duration_ms,
        result.iterations,
        result.tool_calls.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_user_prompt_basic() {
        let prompt = build_user_prompt("#42", None, false);
        assert!(prompt.contains("Resolve this GitHub issue: #42"));
        assert!(!prompt.contains("DRY RUN"));
        assert!(!prompt.contains("Repository"));
    }

    #[test]
    fn test_build_user_prompt_with_repo_and_dry_run() {
        let prompt = build_user_prompt(
            "https://github.com/user/repo/issues/123",
            Some("https://github.com/user/repo"),
            true,
        );
        assert!(prompt.contains("issues/123"));
        assert!(prompt.contains("Repository: https://github.com/user/repo"));
        assert!(prompt.contains("DRY RUN"));
    }

    #[test]
    fn test_load_workflow_prompt_fallback() {
        // When file doesn't exist, should return fallback
        let prompt = load_workflow_prompt();
        assert!(!prompt.is_empty());
    }
}
