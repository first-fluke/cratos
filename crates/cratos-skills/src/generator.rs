//! Skill generator for creating skills from detected patterns
//!
//! This module generates skill definitions from patterns detected by the analyzer.

use crate::analyzer::DetectedPattern;
use crate::error::{Error, Result};
use crate::skill::{ErrorAction, Skill, SkillStep, SkillTrigger};
use serde_json::{json, Value};
use tracing::{debug, info, instrument};
use uuid::Uuid;

/// Configuration for the skill generator
#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    /// Minimum confidence to generate a skill
    pub min_confidence: f32,
    /// Whether to automatically activate generated skills
    pub auto_activate: bool,
    /// Maximum number of keywords to include in trigger
    pub max_keywords: usize,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.7,
            auto_activate: false,
            max_keywords: 5,
        }
    }
}

/// Skill generator for creating skills from patterns
pub struct SkillGenerator {
    config: GeneratorConfig,
}

impl SkillGenerator {
    /// Create a new skill generator with default configuration
    pub fn new() -> Self {
        Self {
            config: GeneratorConfig::default(),
        }
    }

    /// Create a skill generator with custom configuration
    pub fn with_config(config: GeneratorConfig) -> Self {
        Self { config }
    }

    /// Generate a skill from a detected pattern
    #[instrument(skip(self, pattern), fields(pattern_id = %pattern.id))]
    pub fn generate_from_pattern(&self, pattern: &DetectedPattern) -> Result<Skill> {
        if pattern.confidence_score < self.config.min_confidence {
            return Err(Error::Validation(format!(
                "pattern confidence {} below threshold {}",
                pattern.confidence_score, self.config.min_confidence
            )));
        }

        let name = self.generate_name(pattern);
        let description = self.generate_description(pattern);

        let mut skill = Skill::auto_generated(&name, &description, pattern.id);

        // Set trigger from extracted keywords
        let keywords: Vec<String> = pattern
            .extracted_keywords
            .iter()
            .take(self.config.max_keywords)
            .cloned()
            .collect();

        skill.trigger = SkillTrigger::with_keywords(keywords);

        // Generate steps from tool sequence
        skill.steps = self.generate_steps(&pattern.tool_sequence);

        // Generate input schema
        skill.input_schema = Some(self.generate_input_schema(&pattern.tool_sequence));

        if self.config.auto_activate {
            skill.activate();
        }

        info!(
            "Generated skill '{}' from pattern {} (confidence: {:.2})",
            skill.name, pattern.id, pattern.confidence_score
        );

        Ok(skill)
    }

    /// Generate multiple skills from patterns
    #[instrument(skip(self, patterns))]
    pub fn generate_from_patterns(&self, patterns: &[DetectedPattern]) -> Vec<(Skill, Uuid)> {
        let mut results = Vec::new();

        for pattern in patterns {
            match self.generate_from_pattern(pattern) {
                Ok(skill) => {
                    results.push((skill, pattern.id));
                }
                Err(e) => {
                    debug!("Skipping pattern {}: {}", pattern.id, e);
                }
            }
        }

        info!(
            "Generated {} skills from {} patterns",
            results.len(),
            patterns.len()
        );
        results
    }

    /// Generate a name for the skill based on the tool sequence
    fn generate_name(&self, pattern: &DetectedPattern) -> String {
        let tools: Vec<&str> = pattern
            .tool_sequence
            .iter()
            .map(|s| s.as_str())
            .collect();

        // Create a name like "file_read_then_git_commit"
        if tools.len() <= 3 {
            tools.join("_then_")
        } else {
            // For longer sequences, use first and last with count
            format!(
                "{}_to_{}_{}steps",
                tools.first().unwrap_or(&"unknown"),
                tools.last().unwrap_or(&"unknown"),
                tools.len()
            )
        }
    }

    /// Generate a description for the skill
    fn generate_description(&self, pattern: &DetectedPattern) -> String {
        let tools = pattern.tool_sequence.join(" → ");
        let keywords = if pattern.extracted_keywords.is_empty() {
            String::new()
        } else {
            format!(
                " (triggers: {})",
                pattern.extracted_keywords.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
            )
        };

        format!(
            "Auto-generated workflow: {}{}\n\nOccurred {} times with {:.0}% confidence.",
            tools,
            keywords,
            pattern.occurrence_count,
            pattern.confidence_score * 100.0
        )
    }

    /// Generate execution steps from a tool sequence
    fn generate_steps(&self, tool_sequence: &[String]) -> Vec<SkillStep> {
        tool_sequence
            .iter()
            .enumerate()
            .map(|(i, tool_name)| {
                let input_template = self.generate_step_input_template(tool_name);
                let description = self.generate_step_description(tool_name);

                SkillStep::new((i + 1) as u32, tool_name, input_template)
                    .with_description(description)
                    .with_on_error(if i == 0 {
                        ErrorAction::Abort // First step failure aborts
                    } else {
                        ErrorAction::Continue // Later steps can continue
                    })
            })
            .collect()
    }

    /// Generate an input template for a tool
    fn generate_step_input_template(&self, tool_name: &str) -> Value {
        // Common tool input patterns
        match tool_name {
            "file_read" | "file_read_batch" => json!({
                "path": "{{file_path}}"
            }),
            "file_write" | "file_write_batch" => json!({
                "path": "{{file_path}}",
                "content": "{{content}}"
            }),
            "git_status" | "git_diff" | "git_log" => json!({}),
            "git_commit" => json!({
                "message": "{{commit_message}}"
            }),
            "git_push" => json!({
                "remote": "{{remote}}",
                "branch": "{{branch}}"
            }),
            "exec" | "shell" => json!({
                "command": "{{command}}"
            }),
            "http_get" | "http_post" => json!({
                "url": "{{url}}"
            }),
            "github_create_pr" => json!({
                "title": "{{pr_title}}",
                "body": "{{pr_body}}",
                "head": "{{head_branch}}",
                "base": "{{base_branch}}"
            }),
            _ => json!({
                "input": "{{input}}"
            }),
        }
    }

    /// Generate a description for a step
    fn generate_step_description(&self, tool_name: &str) -> String {
        match tool_name {
            "file_read" => "Read file contents".to_string(),
            "file_write" => "Write content to file".to_string(),
            "git_status" => "Check git status".to_string(),
            "git_diff" => "Show git diff".to_string(),
            "git_log" => "Show git log".to_string(),
            "git_commit" => "Create a git commit".to_string(),
            "git_push" => "Push to remote".to_string(),
            "exec" | "shell" => "Execute shell command".to_string(),
            "http_get" => "HTTP GET request".to_string(),
            "http_post" => "HTTP POST request".to_string(),
            "github_create_pr" => "Create GitHub pull request".to_string(),
            _ => format!("Execute {}", tool_name),
        }
    }

    /// Generate a JSON schema for the skill input
    fn generate_input_schema(&self, tool_sequence: &[String]) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        // Collect all unique variables from the tool sequence
        for tool_name in tool_sequence {
            let vars = self.get_tool_variables(tool_name);
            for (var_name, var_type, is_required) in vars {
                if !properties.contains_key(&var_name) {
                    properties.insert(
                        var_name.clone(),
                        json!({
                            "type": var_type,
                            "description": format!("Input for {}", tool_name)
                        }),
                    );
                    if is_required && !required.contains(&var_name) {
                        required.push(var_name);
                    }
                }
            }
        }

        json!({
            "type": "object",
            "properties": properties,
            "required": required
        })
    }

    /// Get variable definitions for a tool
    fn get_tool_variables(&self, tool_name: &str) -> Vec<(String, &'static str, bool)> {
        match tool_name {
            "file_read" | "file_read_batch" => {
                vec![("file_path".to_string(), "string", true)]
            }
            "file_write" | "file_write_batch" => vec![
                ("file_path".to_string(), "string", true),
                ("content".to_string(), "string", true),
            ],
            "git_commit" => vec![("commit_message".to_string(), "string", true)],
            "git_push" => vec![
                ("remote".to_string(), "string", false),
                ("branch".to_string(), "string", false),
            ],
            "exec" | "shell" => vec![("command".to_string(), "string", true)],
            "http_get" | "http_post" => vec![("url".to_string(), "string", true)],
            "github_create_pr" => vec![
                ("pr_title".to_string(), "string", true),
                ("pr_body".to_string(), "string", false),
                ("head_branch".to_string(), "string", true),
                ("base_branch".to_string(), "string", false),
            ],
            _ => vec![("input".to_string(), "string", false)],
        }
    }
}

impl Default for SkillGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::PatternStatus;
    use chrono::Utc;

    fn create_test_pattern() -> DetectedPattern {
        DetectedPattern {
            id: Uuid::new_v4(),
            tool_sequence: vec!["file_read".to_string(), "git_commit".to_string()],
            occurrence_count: 10,
            confidence_score: 0.8,
            extracted_keywords: vec!["read".to_string(), "commit".to_string()],
            sample_inputs: vec!["read the file and commit".to_string()],
            status: PatternStatus::Detected,
            converted_skill_id: None,
            detected_at: Utc::now(),
        }
    }

    #[test]
    fn test_generate_from_pattern() {
        let generator = SkillGenerator::new();
        let pattern = create_test_pattern();

        let skill = generator.generate_from_pattern(&pattern).unwrap();

        assert_eq!(skill.name, "file_read_then_git_commit");
        assert_eq!(skill.steps.len(), 2);
        assert_eq!(skill.steps[0].tool_name, "file_read");
        assert_eq!(skill.steps[1].tool_name, "git_commit");
        assert_eq!(skill.metadata.source_pattern_id, Some(pattern.id));
    }

    #[test]
    fn test_generate_name_short() {
        let generator = SkillGenerator::new();
        let mut pattern = create_test_pattern();

        pattern.tool_sequence = vec!["a".to_string(), "b".to_string()];
        let skill = generator.generate_from_pattern(&pattern).unwrap();
        assert_eq!(skill.name, "a_then_b");
    }

    #[test]
    fn test_generate_name_long() {
        let generator = SkillGenerator::new();
        let mut pattern = create_test_pattern();

        pattern.tool_sequence = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ];
        let skill = generator.generate_from_pattern(&pattern).unwrap();
        assert_eq!(skill.name, "a_to_e_5steps");
    }

    #[test]
    fn test_low_confidence_rejected() {
        let generator = SkillGenerator::with_config(GeneratorConfig {
            min_confidence: 0.9,
            ..Default::default()
        });

        let mut pattern = create_test_pattern();
        pattern.confidence_score = 0.5;

        let result = generator.generate_from_pattern(&pattern);
        assert!(result.is_err());
    }

    #[test]
    fn test_auto_activate() {
        let generator = SkillGenerator::with_config(GeneratorConfig {
            auto_activate: true,
            ..Default::default()
        });

        let pattern = create_test_pattern();
        let skill = generator.generate_from_pattern(&pattern).unwrap();

        assert!(skill.is_active());
    }

    #[test]
    fn test_input_schema_generation() {
        let generator = SkillGenerator::new();
        let pattern = create_test_pattern();

        let skill = generator.generate_from_pattern(&pattern).unwrap();
        let schema = skill.input_schema.unwrap();

        assert!(schema.get("properties").is_some());
        assert!(schema["properties"].get("file_path").is_some());
        assert!(schema["properties"].get("commit_message").is_some());
    }

    #[test]
    fn test_generate_from_patterns() {
        let generator = SkillGenerator::new();
        let patterns = vec![
            create_test_pattern(),
            {
                let mut p = create_test_pattern();
                p.id = Uuid::new_v4();
                p.tool_sequence = vec!["git_status".to_string()];
                p
            },
        ];

        let results = generator.generate_from_patterns(&patterns);
        assert_eq!(results.len(), 2);
    }
}
