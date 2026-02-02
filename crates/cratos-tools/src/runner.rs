//! Runner - Tool execution engine
//!
//! This module provides the execution engine for tools, including:
//! - Timeout handling
//! - Sandboxing (conceptual)
//! - Logging and metrics

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, ToolRegistry, ToolResult};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, error, instrument, warn};

/// Configuration for the tool runner
#[derive(Debug, Clone)]
pub struct RunnerConfig {
    /// Default timeout for tool execution
    pub default_timeout: Duration,
    /// Maximum timeout allowed
    pub max_timeout: Duration,
    /// Whether to enforce sandboxing
    pub sandbox_enabled: bool,
    /// Working directory for file operations
    pub working_dir: Option<String>,
    /// Whether to allow high-risk tools
    pub allow_high_risk: bool,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            max_timeout: Duration::from_secs(300),
            sandbox_enabled: true,
            working_dir: None,
            allow_high_risk: false,
        }
    }
}

impl RunnerConfig {
    /// Create a new configuration with default timeout
    #[must_use]
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            default_timeout,
            ..Default::default()
        }
    }

    /// Set the default timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Set the maximum timeout
    #[must_use]
    pub fn with_max_timeout(mut self, max_timeout: Duration) -> Self {
        self.max_timeout = max_timeout;
        self
    }

    /// Enable or disable sandboxing
    #[must_use]
    pub fn with_sandbox(mut self, enabled: bool) -> Self {
        self.sandbox_enabled = enabled;
        self
    }

    /// Set the working directory
    #[must_use]
    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Allow or disallow high-risk tools
    #[must_use]
    pub fn with_high_risk(mut self, allowed: bool) -> Self {
        self.allow_high_risk = allowed;
        self
    }
}

/// Options for a single tool execution
#[derive(Debug, Clone, Default)]
pub struct ExecutionOptions {
    /// Custom timeout for this execution
    pub timeout: Option<Duration>,
    /// Skip validation
    pub skip_validation: bool,
    /// Dry run (validate but don't execute)
    pub dry_run: bool,
}

impl ExecutionOptions {
    /// Create options with a specific timeout
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout: Some(timeout),
            ..Default::default()
        }
    }

    /// Create dry-run options
    #[must_use]
    pub fn dry_run() -> Self {
        Self {
            dry_run: true,
            ..Default::default()
        }
    }
}

/// Tool execution result with additional metadata
#[derive(Debug)]
pub struct ExecutionResult {
    /// The tool result
    pub result: ToolResult,
    /// Tool name
    pub tool_name: String,
    /// Whether this was a dry run
    pub dry_run: bool,
    /// Whether approval was required
    pub required_approval: bool,
}

/// Tool runner for executing tools with safety measures
pub struct ToolRunner {
    registry: Arc<ToolRegistry>,
    config: RunnerConfig,
}

impl ToolRunner {
    /// Create a new tool runner
    #[must_use]
    pub fn new(registry: Arc<ToolRegistry>, config: RunnerConfig) -> Self {
        Self { registry, config }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults(registry: Arc<ToolRegistry>) -> Self {
        Self::new(registry, RunnerConfig::default())
    }

    /// Get the registry
    #[must_use]
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &RunnerConfig {
        &self.config
    }

    /// Execute a tool by name
    #[instrument(skip(self, input), fields(tool = %tool_name))]
    pub async fn execute(
        &self,
        tool_name: &str,
        input: serde_json::Value,
    ) -> Result<ExecutionResult> {
        self.execute_with_options(tool_name, input, ExecutionOptions::default())
            .await
    }

    /// Execute a tool with custom options
    #[instrument(skip(self, input, options), fields(tool = %tool_name))]
    pub async fn execute_with_options(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        options: ExecutionOptions,
    ) -> Result<ExecutionResult> {
        // Get the tool
        let tool = self
            .registry
            .get(tool_name)
            .ok_or_else(|| Error::NotFound(tool_name.to_string()))?;

        let definition = tool.definition();

        // Check if tool is enabled
        if !definition.enabled {
            return Err(Error::PermissionDenied(format!(
                "Tool '{}' is disabled",
                tool_name
            )));
        }

        // Check risk level
        let requires_approval = definition.risk_level.requires_approval();
        if !self.config.allow_high_risk && definition.risk_level == RiskLevel::High {
            warn!(tool = %tool_name, "High-risk tool execution blocked");
            return Err(Error::PermissionDenied(format!(
                "High-risk tool '{}' requires approval",
                tool_name
            )));
        }

        // Validate input
        if !options.skip_validation {
            tool.validate_input(&input)?;
        }

        // Dry run - return without executing
        if options.dry_run {
            debug!(tool = %tool_name, "Dry run - skipping execution");
            return Ok(ExecutionResult {
                result: ToolResult::success(
                    serde_json::json!({
                        "dry_run": true,
                        "would_execute": tool_name,
                        "input": input
                    }),
                    0,
                ),
                tool_name: tool_name.to_string(),
                dry_run: true,
                required_approval: requires_approval,
            });
        }

        // Determine timeout
        let execution_timeout = options
            .timeout
            .unwrap_or(self.config.default_timeout)
            .min(self.config.max_timeout);

        // Execute with timeout
        let start = Instant::now();
        debug!(tool = %tool_name, timeout_ms = %execution_timeout.as_millis(), "Executing tool");

        let result = match timeout(execution_timeout, tool.execute(input)).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                let duration = start.elapsed().as_millis() as u64;
                error!(tool = %tool_name, error = %e, "Tool execution failed");
                ToolResult::failure(e.to_string(), duration)
            }
            Err(_) => {
                let duration = start.elapsed().as_millis() as u64;
                warn!(tool = %tool_name, timeout_ms = %execution_timeout.as_millis(), "Tool execution timed out");
                return Err(Error::Timeout(duration));
            }
        };

        debug!(
            tool = %tool_name,
            success = %result.success,
            duration_ms = %result.duration_ms,
            "Tool execution completed"
        );

        Ok(ExecutionResult {
            result,
            tool_name: tool_name.to_string(),
            dry_run: false,
            required_approval: requires_approval,
        })
    }

    /// Execute multiple tools in sequence
    #[instrument(skip(self, calls))]
    pub async fn execute_sequence(
        &self,
        calls: Vec<(String, serde_json::Value)>,
    ) -> Vec<Result<ExecutionResult>> {
        let mut results = Vec::with_capacity(calls.len());

        for (tool_name, input) in calls {
            let result = self.execute(&tool_name, input).await;
            let should_stop = result.is_err();
            results.push(result);

            if should_stop {
                break;
            }
        }

        results
    }

    /// Execute multiple tools in parallel
    #[instrument(skip(self, calls))]
    pub async fn execute_parallel(
        &self,
        calls: Vec<(String, serde_json::Value)>,
    ) -> Vec<Result<ExecutionResult>> {
        let futures: Vec<_> = calls
            .into_iter()
            .map(|(tool_name, input)| {
                let runner = self.clone();
                async move { runner.execute(&tool_name, input).await }
            })
            .collect();

        futures::future::join_all(futures).await
    }

    /// Check if a tool can be executed (without actually executing)
    pub fn can_execute(&self, tool_name: &str) -> Result<bool> {
        let tool = self
            .registry
            .get(tool_name)
            .ok_or_else(|| Error::NotFound(tool_name.to_string()))?;

        let definition = tool.definition();

        if !definition.enabled {
            return Ok(false);
        }

        if !self.config.allow_high_risk && definition.risk_level == RiskLevel::High {
            return Ok(false);
        }

        Ok(true)
    }
}

impl Clone for ToolRunner {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_config() {
        let config = RunnerConfig::new(Duration::from_secs(60))
            .with_max_timeout(Duration::from_secs(120))
            .with_sandbox(false)
            .with_working_dir("/tmp")
            .with_high_risk(true);

        assert_eq!(config.default_timeout, Duration::from_secs(60));
        assert_eq!(config.max_timeout, Duration::from_secs(120));
        assert!(!config.sandbox_enabled);
        assert_eq!(config.working_dir, Some("/tmp".to_string()));
        assert!(config.allow_high_risk);
    }

    #[test]
    fn test_execution_options() {
        let opts = ExecutionOptions::with_timeout(Duration::from_secs(10));
        assert_eq!(opts.timeout, Some(Duration::from_secs(10)));
        assert!(!opts.dry_run);

        let dry_opts = ExecutionOptions::dry_run();
        assert!(dry_opts.dry_run);
    }
}
