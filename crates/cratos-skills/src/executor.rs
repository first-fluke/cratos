//! Skill executor for running skill workflows
//!
//! This module handles the execution of skill steps, including
//! variable interpolation and error handling.

use crate::error::{Error, Result};
use crate::skill::{ErrorAction, Skill, SkillStep};
use crate::store::SkillStore;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// Trait for tool execution backends
///
/// This allows the executor to work with different tool systems.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool with the given input
    async fn execute_tool(
        &self,
        tool_name: &str,
        input: Value,
    ) -> std::result::Result<Value, String>;

    /// Check if a tool exists
    fn has_tool(&self, tool_name: &str) -> bool;

    /// Get tool names
    fn tool_names(&self) -> Vec<String>;
}

/// Result of executing a single step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Step number
    pub step: u32,
    /// Tool name
    pub tool_name: String,
    /// Whether the step succeeded
    pub success: bool,
    /// Output (if successful)
    pub output: Option<Value>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Result of executing a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionResult {
    /// Skill ID
    pub skill_id: Uuid,
    /// Skill name
    pub skill_name: String,
    /// Whether the overall execution succeeded
    pub success: bool,
    /// Results for each step
    pub step_results: Vec<StepResult>,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Final output (from last successful step)
    pub final_output: Option<Value>,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Configuration for the skill executor
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum number of retries for a single step
    pub max_retries: u32,
    /// Whether to run in dry-run mode (no actual execution)
    pub dry_run: bool,
    /// Whether to continue on step failure
    pub continue_on_failure: bool,
    /// Timeout per step in milliseconds
    pub step_timeout_ms: u64,
    /// Maximum variable value length (security: prevent memory exhaustion)
    pub max_variable_value_length: usize,
    /// Maximum number of steps per skill (security: prevent infinite loops)
    pub max_steps_per_skill: usize,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            dry_run: false,
            continue_on_failure: false,
            step_timeout_ms: 60_000,          // 1 minute
            max_variable_value_length: 100_000, // 100KB max per variable
            max_steps_per_skill: 50,           // Max 50 steps per skill
        }
    }
}

/// Skill executor for running skill workflows
pub struct SkillExecutor<T: ToolExecutor> {
    tool_executor: T,
    store: Option<SkillStore>,
    config: ExecutorConfig,
}

impl<T: ToolExecutor> SkillExecutor<T> {
    /// Create a new executor with a tool backend
    pub fn new(tool_executor: T) -> Self {
        Self {
            tool_executor,
            store: None,
            config: ExecutorConfig::default(),
        }
    }

    /// Create an executor with a store for tracking
    pub fn with_store(tool_executor: T, store: SkillStore) -> Self {
        Self {
            tool_executor,
            store: Some(store),
            config: ExecutorConfig::default(),
        }
    }

    /// Set custom configuration
    pub fn with_config(mut self, config: ExecutorConfig) -> Self {
        self.config = config;
        self
    }

    /// Execute a skill with the given variables
    #[instrument(skip(self, skill, variables), fields(skill_id = %skill.id, skill_name = %skill.name))]
    pub async fn execute(
        &self,
        skill: &Skill,
        variables: &HashMap<String, Value>,
    ) -> Result<SkillExecutionResult> {
        // SECURITY: Validate skill step count
        if skill.steps.len() > self.config.max_steps_per_skill {
            return Err(Error::Validation(format!(
                "Skill has too many steps ({} > {})",
                skill.steps.len(),
                self.config.max_steps_per_skill
            )));
        }

        // SECURITY: Validate variable values
        for (key, value) in variables {
            let value_str = value.to_string();
            if value_str.len() > self.config.max_variable_value_length {
                return Err(Error::Validation(format!(
                    "Variable '{}' value too large ({} > {})",
                    key,
                    value_str.len(),
                    self.config.max_variable_value_length
                )));
            }
        }

        let start = Instant::now();
        let mut step_results = Vec::new();
        let mut last_output: Option<Value> = None;
        let mut overall_success = true;
        let mut final_error: Option<String> = None;

        // Context for variable interpolation (includes outputs from previous steps)
        let mut context = variables.clone();

        info!(
            "Executing skill '{}' with {} steps (dry_run: {})",
            skill.name,
            skill.steps.len(),
            self.config.dry_run
        );

        for step in &skill.steps {
            let step_result = self.execute_step(step, &context).await;

            match &step_result {
                Ok(result) if result.success => {
                    // Store output for use in subsequent steps
                    if let Some(ref output) = result.output {
                        context.insert(format!("step{}_output", step.order), output.clone());
                        last_output = Some(output.clone());
                    }
                    step_results.push(result.clone());
                }
                Ok(result) => {
                    // Step failed
                    step_results.push(result.clone());

                    match step.on_error {
                        ErrorAction::Abort => {
                            overall_success = false;
                            final_error = result.error.clone();
                            warn!("Step {} failed, aborting skill execution", step.order);
                            break;
                        }
                        ErrorAction::Continue => {
                            warn!("Step {} failed, continuing with next step", step.order);
                        }
                        ErrorAction::Retry => {
                            // Already handled in execute_step
                            overall_success = false;
                            final_error = result.error.clone();
                            break;
                        }
                    }
                }
                Err(e) => {
                    overall_success = false;
                    final_error = Some(e.to_string());
                    step_results.push(StepResult {
                        step: step.order,
                        tool_name: step.tool_name.clone(),
                        success: false,
                        output: None,
                        error: Some(e.to_string()),
                        duration_ms: 0,
                    });
                    break;
                }
            }
        }

        let total_duration = start.elapsed().as_millis() as u64;

        let result = SkillExecutionResult {
            skill_id: skill.id,
            skill_name: skill.name.clone(),
            success: overall_success,
            step_results: step_results.clone(),
            total_duration_ms: total_duration,
            final_output: last_output,
            error: final_error,
        };

        // Record execution if we have a store
        if let Some(ref store) = self.store {
            let step_results_json: Vec<Value> = step_results
                .iter()
                .map(|r| serde_json::to_value(r).unwrap_or(json!({})))
                .collect();

            if let Err(e) = store
                .record_skill_execution(
                    skill.id,
                    None, // No execution_id for now
                    overall_success,
                    Some(total_duration),
                    &step_results_json,
                )
                .await
            {
                error!("Failed to record skill execution: {}", e);
            }
        }

        info!(
            "Skill '{}' execution completed: success={}, duration={}ms",
            skill.name, overall_success, total_duration
        );

        Ok(result)
    }

    /// Execute a single step
    async fn execute_step(
        &self,
        step: &SkillStep,
        context: &HashMap<String, Value>,
    ) -> Result<StepResult> {
        let start = Instant::now();

        // Interpolate variables in the input template
        let input = Self::interpolate_variables(&step.input_template, context)?;

        debug!(
            "Executing step {}: {} with input {:?}",
            step.order, step.tool_name, input
        );

        // Check if tool exists
        if !self.tool_executor.has_tool(&step.tool_name) {
            return Ok(StepResult {
                step: step.order,
                tool_name: step.tool_name.clone(),
                success: false,
                output: None,
                error: Some(format!("Tool '{}' not found", step.tool_name)),
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Execute (with retries if configured)
        let mut last_error = None;
        let max_attempts = if step.on_error == ErrorAction::Retry {
            step.max_retries.max(1) as usize
        } else {
            1
        };

        for attempt in 1..=max_attempts {
            if self.config.dry_run {
                // In dry-run mode, just pretend it succeeded
                return Ok(StepResult {
                    step: step.order,
                    tool_name: step.tool_name.clone(),
                    success: true,
                    output: Some(json!({"dry_run": true, "input": input})),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                });
            }

            match self.tool_executor.execute_tool(&step.tool_name, input.clone()).await {
                Ok(output) => {
                    return Ok(StepResult {
                        step: step.order,
                        tool_name: step.tool_name.clone(),
                        success: true,
                        output: Some(output),
                        error: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                    });
                }
                Err(e) => {
                    last_error = Some(e.clone());
                    if attempt < max_attempts {
                        warn!(
                            "Step {} attempt {}/{} failed: {}",
                            step.order, attempt, max_attempts, e
                        );
                    }
                }
            }
        }

        Ok(StepResult {
            step: step.order,
            tool_name: step.tool_name.clone(),
            success: false,
            output: None,
            error: last_error,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Interpolate variables in a JSON value using {{variable}} syntax
    fn interpolate_variables(
        template: &Value,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        match template {
            Value::String(s) => {
                // Replace {{variable}} patterns
                let mut result = s.clone();
                let re = regex::Regex::new(r"\{\{(\w+)\}\}")
                    .map_err(|e| Error::Execution(e.to_string()))?;

                for cap in re.captures_iter(s) {
                    let var_name = &cap[1];
                    if let Some(value) = context.get(var_name) {
                        let replacement = match value {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            _ => value.to_string(),
                        };
                        result = result.replace(&cap[0], &replacement);
                    }
                }
                Ok(Value::String(result))
            }
            Value::Object(obj) => {
                let mut new_obj = serde_json::Map::new();
                for (k, v) in obj {
                    new_obj.insert(k.clone(), Self::interpolate_variables(v, context)?);
                }
                Ok(Value::Object(new_obj))
            }
            Value::Array(arr) => {
                let new_arr: Vec<Value> = arr
                    .iter()
                    .map(|v| Self::interpolate_variables(v, context))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::Array(new_arr))
            }
            // Other types pass through unchanged
            other => Ok(other.clone()),
        }
    }
}

/// A mock tool executor for testing
#[cfg(test)]
#[allow(missing_docs)]
pub struct MockToolExecutor {
    tools: HashMap<String, Box<dyn Fn(Value) -> std::result::Result<Value, String> + Send + Sync>>,
}

#[cfg(test)]
#[allow(missing_docs)]
impl MockToolExecutor {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn add_tool<F>(&mut self, name: &str, handler: F)
    where
        F: Fn(Value) -> std::result::Result<Value, String> + Send + Sync + 'static,
    {
        self.tools.insert(name.to_string(), Box::new(handler));
    }
}

#[cfg(test)]
#[async_trait]
impl ToolExecutor for MockToolExecutor {
    async fn execute_tool(
        &self,
        tool_name: &str,
        input: Value,
    ) -> std::result::Result<Value, String> {
        match self.tools.get(tool_name) {
            Some(handler) => handler(input),
            None => Err(format!("Tool '{}' not found", tool_name)),
        }
    }

    fn has_tool(&self, tool_name: &str) -> bool {
        self.tools.contains_key(tool_name)
    }

    fn tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::{SkillCategory, SkillStep};

    fn create_test_skill() -> Skill {
        Skill::new("test_skill", "Test", SkillCategory::Custom)
            .with_step(SkillStep::new(
                1,
                "file_read",
                json!({"path": "{{file_path}}"}),
            ))
            .with_step(SkillStep::new(
                2,
                "transform",
                json!({"input": "{{step1_output}}"}),
            ))
    }

    fn create_mock_executor() -> MockToolExecutor {
        let mut executor = MockToolExecutor::new();

        executor.add_tool("file_read", |input| {
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
            Ok(json!({"content": format!("content of {}", path)}))
        });

        executor.add_tool("transform", |input| {
            Ok(json!({"transformed": input}))
        });

        executor
    }

    #[tokio::test]
    async fn test_execute_skill() {
        let mock = create_mock_executor();
        let executor = SkillExecutor::new(mock);
        let skill = create_test_skill();

        let mut variables = HashMap::new();
        variables.insert("file_path".to_string(), json!("/test/file.txt"));

        let result = executor.execute(&skill, &variables).await.unwrap();

        assert!(result.success);
        assert_eq!(result.step_results.len(), 2);
        assert!(result.step_results[0].success);
        assert!(result.step_results[1].success);
    }

    #[tokio::test]
    async fn test_dry_run_mode() {
        let mock = create_mock_executor();
        let executor = SkillExecutor::new(mock).with_config(ExecutorConfig {
            dry_run: true,
            ..Default::default()
        });
        let skill = create_test_skill();

        let variables = HashMap::new();
        let result = executor.execute(&skill, &variables).await.unwrap();

        assert!(result.success);
        // In dry-run mode, output should indicate it
        assert!(result.step_results[0]
            .output
            .as_ref()
            .map(|v| v.get("dry_run").is_some())
            .unwrap_or(false));
    }

    #[tokio::test]
    async fn test_variable_interpolation() {
        let template = json!({
            "path": "{{file_path}}",
            "options": {
                "name": "{{name}}"
            },
            "list": ["{{item1}}", "{{item2}}"]
        });

        let mut context = HashMap::new();
        context.insert("file_path".to_string(), json!("/path/to/file"));
        context.insert("name".to_string(), json!("test"));
        context.insert("item1".to_string(), json!("a"));
        context.insert("item2".to_string(), json!("b"));

        let result = SkillExecutor::<MockToolExecutor>::interpolate_variables(&template, &context).unwrap();

        assert_eq!(result["path"], "/path/to/file");
        assert_eq!(result["options"]["name"], "test");
        assert_eq!(result["list"][0], "a");
        assert_eq!(result["list"][1], "b");
    }

    #[tokio::test]
    async fn test_step_failure_handling() {
        let mut mock = MockToolExecutor::new();
        mock.add_tool("failing_tool", |_| Err("Tool failed".to_string()));

        let executor = SkillExecutor::new(mock);

        let skill = Skill::new("fail_skill", "Test", SkillCategory::Custom)
            .with_step(SkillStep::new(1, "failing_tool", json!({})));

        let result = executor.execute(&skill, &HashMap::new()).await.unwrap();

        assert!(!result.success);
        assert!(!result.step_results[0].success);
        assert!(result.step_results[0].error.is_some());
    }

    #[tokio::test]
    async fn test_missing_tool() {
        let mock = MockToolExecutor::new(); // No tools registered
        let executor = SkillExecutor::new(mock);

        let skill = Skill::new("test", "Test", SkillCategory::Custom)
            .with_step(SkillStep::new(1, "nonexistent", json!({})));

        let result = executor.execute(&skill, &HashMap::new()).await.unwrap();

        assert!(!result.success);
        assert!(result.step_results[0].error.as_ref().unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_security_too_many_steps() {
        let mock = create_mock_executor();
        let config = ExecutorConfig {
            max_steps_per_skill: 2,
            ..Default::default()
        };
        let executor = SkillExecutor::new(mock).with_config(config);

        // Create skill with 3 steps (exceeds limit of 2)
        let skill = Skill::new("test", "Test", SkillCategory::Custom)
            .with_step(SkillStep::new(1, "file_read", json!({})))
            .with_step(SkillStep::new(2, "transform", json!({})))
            .with_step(SkillStep::new(3, "transform", json!({})));

        let result = executor.execute(&skill, &HashMap::new()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too many steps"));
    }

    #[tokio::test]
    async fn test_security_variable_too_large() {
        let mock = create_mock_executor();
        let config = ExecutorConfig {
            max_variable_value_length: 100,
            ..Default::default()
        };
        let executor = SkillExecutor::new(mock).with_config(config);

        let skill = create_test_skill();
        let mut variables = HashMap::new();
        // Create a value larger than 100 bytes
        variables.insert("file_path".to_string(), json!("a".repeat(200)));

        let result = executor.execute(&skill, &variables).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too large"));
    }
}
