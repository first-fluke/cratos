//! Planner - Natural language to execution plan conversion
//!
//! This module provides the planning functionality that converts
//! natural language requests into executable plans with tool calls.

use crate::error::{Error, Result};
use cratos_llm::{
    CompletionRequest, LlmProvider, Message, ToolCall, ToolChoice, ToolCompletionRequest,
    ToolCompletionResponse, ToolDefinition,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, instrument};

/// Default system prompt for the planner
pub const DEFAULT_SYSTEM_PROMPT: &str = r#"You are **Cratos**, an AI agent running on the user's LOCAL machine.
Your LLM backend is **{provider_name}** (model: {model_name}).
You are NOT any other AI model. If asked what model you use, answer with your actual backend shown above.

## CRITICAL: ACT, DON'T TALK
When the user asks you to DO something (check files, find TODOs, build code, modify files, etc.), you MUST use tools immediately. NEVER say "I'll check" or "tell me the path" — just DO IT with the tools available to you. The user is on their phone and cannot provide paths or run commands themselves.

**WRONG**: "I'll check the project for TODOs. Please tell me the path."
**RIGHT**: Call `exec` with command="grep" args=["-r", "TODO", "/Volumes/gahyun_ex/projects/cratos/src/"]

## Machine Info
- OS: macOS (Darwin)
- User: gahyun
- Home: /Users/gahyun/
- Projects: /Volumes/gahyun_ex/projects/
  - Cratos (Rust): /Volumes/gahyun_ex/projects/cratos/
  - Test projects: /Volumes/gahyun_ex/projects/test/
- To discover other projects: `file_list` on `/Volumes/gahyun_ex/projects/`

## Rules
- Respond in the SAME LANGUAGE the user writes in (Korean → Korean, English → English)
- Be concise. No filler text.
- Use function calling. NEVER simulate tool calls as text or XML tags.
- If you don't know a path, use `file_list` to discover it. NEVER ask the user for paths.
- If a tool fails, explain what went wrong and try an alternative approach.

## Tools
- `exec`: Run commands. Example: command="cargo" args=["check"] cwd="/Volumes/gahyun_ex/projects/cratos"
- `file_read`: Read a file
- `file_write`: Write/overwrite a file (provide COMPLETE content)
- `file_list`: List directory contents (use this to discover paths!)
- `git_status`, `git_diff`, `git_commit`, `git_branch`, `git_push`: Git ops
- `github_api`: GitHub API
- `http_get`, `http_post`: HTTP requests
- `config`: Cratos configuration
- `browser`: Browser automation

## When to use tools (ALWAYS use tools for these)
- "TODO 확인해" → `exec` grep -r TODO in project
- "빌드 됐어?" → `exec` ls -la on binary or cargo build
- "파일 보여줘" → `file_list` then `file_read`
- "코드 수정해" → `file_read` → `file_write` → `exec` cargo check
- "프로세스 확인" → `exec` ps aux
- Only respond WITHOUT tools for: greetings, general knowledge, opinions

## Personas (@mention to switch)
Cratos(Orchestrator), Athena(PM), Sindri(Dev), Heimdall(QA), Mimir(Research), Thor(DevOps), Odin(PO)
"#;

/// Configuration for the planner
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    /// System prompt
    pub system_prompt: String,
    /// Maximum iterations for tool calling
    pub max_iterations: usize,
    /// Whether to include tool definitions in prompts
    pub include_tools: bool,
    /// Default model to use
    pub default_model: Option<String>,
    /// Temperature for generation
    pub temperature: Option<f32>,
    /// Maximum tokens for response
    pub max_tokens: Option<u32>,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            max_iterations: 10,
            include_tools: true,
            default_model: None,
            temperature: Some(0.7),
            max_tokens: Some(4096),
        }
    }
}

impl PlannerConfig {
    /// Create a new configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the system prompt
    #[must_use]
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Inject LLM provider info into the system prompt template
    #[must_use]
    pub fn with_provider_info(mut self, provider_name: &str, model_name: &str) -> Self {
        self.system_prompt = self
            .system_prompt
            .replace("{provider_name}", provider_name)
            .replace("{model_name}", model_name);
        self
    }

    /// Set maximum iterations
    #[must_use]
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set whether to include tools
    #[must_use]
    pub fn with_tools(mut self, include: bool) -> Self {
        self.include_tools = include;
        self
    }

    /// Set the default model
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Set the temperature
    #[must_use]
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }
}

/// Result of a planning step
#[derive(Debug, Clone)]
pub enum PlanStep {
    /// Direct response (no tool calls needed)
    Response(String),
    /// Tool calls to execute
    ToolCalls(Vec<ToolCall>),
    /// Error occurred
    Error(String),
}

/// Complete plan response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanResponse {
    /// Text content from the response
    pub content: Option<String>,
    /// Tool calls requested
    pub tool_calls: Vec<ToolCall>,
    /// Whether this is a final response
    pub is_final: bool,
    /// Finish reason from the model
    pub finish_reason: Option<String>,
    /// Model used
    pub model: String,
}

impl PlanResponse {
    /// Check if there are tool calls
    #[must_use]
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    /// Check if this is just a text response
    #[must_use]
    pub fn is_text_only(&self) -> bool {
        self.tool_calls.is_empty() && self.content.is_some()
    }
}

/// Planner for converting natural language to execution plans
pub struct Planner {
    provider: Arc<dyn LlmProvider>,
    config: PlannerConfig,
}

impl Planner {
    /// Create a new planner
    #[must_use]
    pub fn new(provider: Arc<dyn LlmProvider>, config: PlannerConfig) -> Self {
        Self { provider, config }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults(provider: Arc<dyn LlmProvider>) -> Self {
        Self::new(provider, PlannerConfig::default())
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &PlannerConfig {
        &self.config
    }

    /// Plan a single step with the given messages and tools
    #[instrument(skip(self, messages, tools))]
    pub async fn plan_step(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<PlanResponse> {
        // Build messages with system prompt
        let mut full_messages = vec![Message::system(&self.config.system_prompt)];
        full_messages.extend(messages.iter().cloned());

        let model = self
            .config
            .default_model
            .clone()
            .unwrap_or_else(|| self.provider.default_model().to_string());

        if tools.is_empty() || !self.config.include_tools {
            // Simple completion without tools
            let request = CompletionRequest {
                messages: full_messages,
                model,
                max_tokens: self.config.max_tokens,
                temperature: self.config.temperature,
                stop: None,
            };

            debug!("Making completion request without tools");

            let response = self.provider.complete(request).await.map_err(Error::Llm)?;

            Ok(PlanResponse {
                content: Some(response.content),
                tool_calls: Vec::new(),
                is_final: true,
                finish_reason: response.finish_reason,
                model: response.model,
            })
        } else {
            // Completion with tools
            let request = ToolCompletionRequest {
                request: CompletionRequest {
                    messages: full_messages,
                    model,
                    max_tokens: self.config.max_tokens,
                    temperature: self.config.temperature,
                    stop: None,
                },
                tools: tools.to_vec(),
                tool_choice: ToolChoice::Auto,
            };

            debug!(
                tool_count = tools.len(),
                "Making completion request with tools"
            );

            let response = self
                .provider
                .complete_with_tools(request)
                .await
                .map_err(Error::Llm)?;

            let is_final = response.tool_calls.is_empty();

            Ok(PlanResponse {
                content: response.content,
                tool_calls: response.tool_calls,
                is_final,
                finish_reason: response.finish_reason,
                model: response.model,
            })
        }
    }

    /// Convert a tool completion response to a plan step
    #[must_use]
    pub fn response_to_step(response: &ToolCompletionResponse) -> PlanStep {
        if !response.tool_calls.is_empty() {
            PlanStep::ToolCalls(response.tool_calls.clone())
        } else if let Some(content) = &response.content {
            PlanStep::Response(content.clone())
        } else {
            PlanStep::Error("No response content or tool calls".to_string())
        }
    }

    /// Maximum characters for tool result content sent back to LLM.
    /// Large outputs (e.g. `ps aux`) cause Gemini to loop instead of summarizing.
    const MAX_TOOL_RESULT_CHARS: usize = 4000;

    /// Build a message from tool execution results
    #[must_use]
    pub fn build_tool_result_messages(
        tool_calls: &[ToolCall],
        results: &[serde_json::Value],
    ) -> Vec<Message> {
        tool_calls
            .iter()
            .zip(results.iter())
            .map(|(call, result)| {
                let content = serde_json::to_string(result).unwrap_or_else(|_| "{}".to_string());
                let content = if content.len() > Self::MAX_TOOL_RESULT_CHARS {
                    let truncated: String = content
                        .char_indices()
                        .take_while(|(i, _)| *i < Self::MAX_TOOL_RESULT_CHARS)
                        .map(|(_, c)| c)
                        .collect();
                    format!("{}...\n[truncated: {} total chars]", truncated, content.len())
                } else {
                    content
                };
                Message::tool_response_named(&call.id, &call.name, content)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_config() {
        let config = PlannerConfig::new()
            .with_max_iterations(5)
            .with_temperature(0.5)
            .with_tools(false);

        assert_eq!(config.max_iterations, 5);
        assert_eq!(config.temperature, Some(0.5));
        assert!(!config.include_tools);
    }

    #[test]
    fn test_plan_response() {
        let response = PlanResponse {
            content: Some("Hello".to_string()),
            tool_calls: Vec::new(),
            is_final: true,
            finish_reason: Some("stop".to_string()),
            model: "test".to_string(),
        };

        assert!(response.is_text_only());
        assert!(!response.has_tool_calls());
    }

    #[test]
    fn test_build_tool_result_messages() {
        let calls = vec![ToolCall {
            id: "call_1".to_string(),
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
        }];
        let results = vec![serde_json::json!({"result": "ok"})];

        let messages = Planner::build_tool_result_messages(&calls, &results);
        assert_eq!(messages.len(), 1);
    }
}
