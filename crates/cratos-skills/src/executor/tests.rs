use super::*;
#[cfg(test)]
type ToolHandler = Box<dyn Fn(Value) -> std::result::Result<Value, String> + Send + Sync>;

/// A mock tool executor for testing
#[cfg(test)]
#[allow(missing_docs)]
#[derive(Default)]
pub struct MockToolExecutor {
    tools: HashMap<String, ToolHandler>,
}

#[cfg(test)]
#[allow(missing_docs)]
impl MockToolExecutor {
    pub fn new() -> Self {
        Self::default()
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

    executor.add_tool("transform", |input| Ok(json!({"transformed": input})));

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

    let result =
        SkillExecutor::<MockToolExecutor>::interpolate_variables(&template, &context).unwrap();

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

    let skill = Skill::new("test", "Test", SkillCategory::Custom).with_step(SkillStep::new(
        1,
        "nonexistent",
        json!({}),
    ));

    let result = executor.execute(&skill, &HashMap::new()).await.unwrap();

    assert!(!result.success);
    assert!(result.step_results[0]
        .error
        .as_ref()
        .unwrap()
        .contains("not found"));
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
