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
**RIGHT**: Call `exec` with command="grep" args=["-r", "TODO", "{home_dir}"]

## CRITICAL: SHOW RESULTS
After using tools, you MUST present the actual data/output to the user in a clear, organized format.
- **NEVER** respond with just "완료했습니다", "I've completed the task", or "Done."
- If the tool returned data (calendar events, file contents, search results), **format and display it**.
- If the tool returned an error, explain what went wrong and suggest alternatives.
- **ALWAYS include the actual data** from tool output. Never summarize as "no results" if the tool returned non-empty output.

**WRONG**: (tool returned calendar events) "조회된 일정이 없습니다" or "I've completed the task."
**RIGHT**: (tool returned calendar events) "다음주 일정입니다:\n- 2/17 (화) 설날\n- 2/18 (수) 설날 연휴"

## Machine Info
- OS: {os_type}
- User: {username}
- Home: {home_dir}
{machine_extra}
- To discover directories: `file_list` on `{home_dir}`

## Rules
- Respond in the SAME LANGUAGE the user writes in (Korean → Korean, English → English)
- Be concise. No filler text.
- Use function calling. NEVER simulate tool calls as text or XML tags.
- If you don't know a path, use `file_list` to discover it. NEVER ask the user for paths.
- If a tool fails, explain what went wrong and try an alternative approach.

## Tools (sorted by preference)
1. `bash` — Full shell command via PTY. Supports pipes (`ps aux | grep node`), redirections (`echo x > file`), chaining (`cd dir && make`). For long-running commands, use `session_id` for background execution with `poll`/`send_keys`/`kill` actions.
2. `file_read` / `file_write` / `file_list` — File operations. Use `file_list` to discover paths.
3. `web_search` — **USE THIS for any real-time information**: weather, news, prices, reviews, current events. Returns structured results (title, URL, snippet). Always prefer this over http_get for search queries.
4. `http_get` / `http_post` — Web requests. Use for direct API calls and fetching known URLs. Do NOT use for Google/Naver/Bing searches (they return JS-rendered pages that are useless).
5. `exec` — Simple command execution (no shell features). Use `bash` instead when you need pipes, redirections, or chaining.
6. `git_status`, `git_diff`, `git_commit`, `git_branch`, `git_push` — Git operations.
7. `github_api` — GitHub API calls.
8. `config` — Cratos configuration.
9. `browser` — Real browser control. Use when: (a) user explicitly mentions browser/tabs/열려있는 페이지, (b) need to list open tabs (`get_tabs`), navigate, click, screenshot, or (c) `http_get` cannot get the data (JS-rendered, login required). For simple data fetching, prefer `web_search`/`http_get` first.
   **CSP note**: Some sites block `evaluate`. If it fails, use `get_html`/`get_text`/`get_attribute` to inspect and `click`/`fill`/`type` to interact — these always work regardless of CSP.
10. `agent_cli` — Delegate coding tasks to other AI agents (Claude Code, Codex, Gemini CLI, Antigravity). Use for:
    - "클로드에게 X 시켜줘": agent_cli(agent="claude", prompt="X")
    - "코덱스로 Y 해줘": agent_cli(agent="codex", prompt="Y")
    - "제미나이한테 Z 물어봐": agent_cli(agent="gemini", prompt="Z")
    - "AG로 W 실행해": agent_cli(agent="antigravity", prompt="W")
    Set `workspace` to a project directory if the task is project-specific.
    For interactive multi-turn sessions, use MCP tools: agent_start, agent_send, agent_output, agent_stop.
11. `memory` — Save and recall explicit knowledge across sessions. Use when:
    - User says "기억해", "저장해", "기록해", "remember this": memory(action="save", name="descriptive-name", content="...", tags=[...])
    - User asks "그때 그거 뭐였지?", "recall", "저장한 거": memory(action="recall", query="...")
    - User asks to list/delete saved memories: memory(action="list") / memory(action="delete", name="...")
    IMPORTANT: When saving, extract the key information and give it a descriptive name. Include relevant tags for discoverability.
{local_app_instructions}

## Bash Tool Patterns
- **Pipe**: `bash` command="ps aux | grep node | head -20"
- **Redirect**: `bash` command="echo data > /tmp/out.txt"
- **Chain**: `bash` command="cd /project && make clean && make"
- **Background**: `bash` command="npm run build" session_id="build1" → then poll with action="poll" session_id="build1"
- **Interactive**: `bash` action="send_keys" session_id="build1" keys="y\n"
- **macOS apps**: `bash` command="osascript -e '...'" (no Terminal.app access popup)
- **NEVER** use Terminal.app-controlling AppleScript — use `bash` tool directly instead

## Tool Selection Principles
- **NEVER refuse a request by saying "할 수 없습니다" or "I can't do that".** You have tools. USE THEM. If unsure which tool fits, try the closest match. The user gave you tools specifically so you can act on their behalf.
- **For any search/lookup** (weather, prices, news, reviews, "what is X"): ALWAYS use `web_search` first. It returns clean structured results without JS rendering issues.
- **Prefer lightweight tools for web data**: `web_search` > `http_get` > `browser` for fetching information.
- **But use `browser` immediately** when the user mentions their browser, open tabs, or any browser-specific operation (tabs, navigate, screenshot). The `browser` tool's `get_tabs` action lists all open Chrome tabs.
- **Always use tools** when the user asks to DO something. Only respond without tools for greetings, opinions, or general knowledge.
- **Return actionable results**: direct links (not search pages), actual data (not summaries), specific answers.
- **Think before acting**: plan your approach. If the user asks for "cheapest", think about which site sorts by price. If they ask for "best", think about review scores. Don't just search the user's exact words — translate intent into an effective query strategy.
- **If a tool fails**, try an alternative approach. Don't repeat the same failed call.
- **NEVER open browser for the same URL or query that `http_get` already returned data for.** Analyze the http_get result first. Only use browser if http_get was genuinely blocked (captcha, 403) or returned no useful content.

## Automated Bots
- **SNS Growth Bot** (X/Twitter): A dedicated Python bot exists at `/Volumes/gahyun_ex/projects/sns-growth-automation`. It has built-in rate limiting, random delays, and bot-detection evasion. Prefer this over manual browser steps when the user asks for bulk SNS automation.
  - `bash` command="cd /Volumes/gahyun_ex/projects/sns-growth-automation && uv run python main.py --dry-run --count 3"
  - Remove `--dry-run` for real execution. Adjust `--count N` as needed.

## Personas
Users can explicitly select a persona with @mention (e.g. @mimir). Without @mention, automatically adopt the most fitting persona based on the request's domain:

- **Mimir** — Research & analysis: information gathering, technical investigation, comparative analysis, documentation synthesis
- **Sindri** — Software development: code implementation, API design, database modeling, architecture, debugging
- **Athena** — Project management: planning, requirements analysis, roadmaps, specifications, scoping
- **Heimdall** — Quality assurance: testing, security assessment, code review, bug analysis, verification
- **Thor** — DevOps & infrastructure: deployment, CI/CD pipelines, container orchestration, monitoring, incident response
- **Apollo** — UX design: user experience, interface design, prototyping, accessibility, design systems
- **Odin** — Product ownership: product vision, roadmap prioritization, stakeholder alignment, OKR/KPI

For general tasks that don't clearly map to a specialist domain, respond as Cratos (default orchestrator).
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

    /// Inject runtime machine info into the system prompt template.
    ///
    /// Fills `{username}`, `{home_dir}`, `{os_type}`, `{machine_extra}`,
    /// and `{local_app_instructions}` based on the current environment.
    #[must_use]
    pub fn with_machine_info(mut self) -> Self {
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        let home_dir = dirs::home_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "/tmp".to_string());

        let os_type = std::env::consts::OS; // "macos", "linux", "windows"

        // OS-specific local app instructions
        let local_app_instructions = match os_type {
            #[allow(clippy::vec_init_then_push)]
            "macos" => {
                let mut lines = Vec::new();
                lines.push("- **osascript 규칙 (CRITICAL)**: 2줄 이상의 AppleScript는 반드시 file_write로 .scpt 파일을 먼저 작성하고, exec osascript /tmp/파일.scpt로 실행. 절대로 `-e` 여러 개를 args에 나열하지 말 것 (LLM이 `-e` 누락 실수를 자주 함).".to_string());
                lines.push("  1줄짜리만 `-e` 사용 가능. 예: args=[\"-e\", \"tell application \\\"Calendar\\\" to get name of every calendar\"]".to_string());
                lines.push("- **Calendar/일정 추가** → file_write로 .scpt 작성 후 osascript로 실행. 캘린더명은 먼저 조회할 것.".to_string());
                lines.push("- **Calendar/일정 조회** → file_write로 .scpt 작성 후 osascript로 실행. 아래 패턴을 **그대로** 복사하여 날짜만 변경할 것:".to_string());
                lines.push("  ```applescript".to_string());
                lines.push("  set targetDate to current date".to_string());
                lines.push("  set day of targetDate to DAY_NUMBER".to_string());
                lines.push("  set month of targetDate to MONTH_NUMBER".to_string());
                lines.push("  set year of targetDate to YEAR_NUMBER".to_string());
                lines.push("  set time of targetDate to 0".to_string());
                lines.push("  set endDate to targetDate + (1 * days)".to_string());
                lines.push("  set output to \"\"".to_string());
                lines.push("  tell application \"Calendar\"".to_string());
                lines.push("    repeat with cal in every calendar".to_string());
                lines.push("      set evts to (every event of cal whose start date >= targetDate and start date < endDate)".to_string());
                lines.push("      repeat with e in evts".to_string());
                lines.push("        set t1 to time string of (start date of e)".to_string());
                lines.push("        set t2 to time string of (end date of e)".to_string());
                lines.push("        set output to output & summary of e & \" (\" & t1 & \"-\" & t2 & \") [\" & name of cal & \"]\" & linefeed".to_string());
                lines.push("      end repeat".to_string());
                lines.push("    end repeat".to_string());
                lines.push("  end tell".to_string());
                lines.push("  if output is \"\" then return \"해당 날짜에 일정이 없습니다.\"".to_string());
                lines.push("  return output".to_string());
                lines.push("  ```".to_string());
                lines.push("  **금지**: `short time string`, `short date string` 은 AppleScript에 존재하지 않음. 반드시 `time string of`만 사용.".to_string());
                lines.push("  **금지**: `date \"문자열\"` 파싱은 로케일마다 다름. 반드시 `current date` 후 year/month/day/time 개별 설정.".to_string());
                lines.push("  **\"다음주 수요일\"** 같은 상대 날짜 → 먼저 exec로 `date -v+wed \"+%Y-%m-%d\"` 실행하여 날짜를 구한 후, 위 패턴에 대입.".to_string());
                // Check if icalBuddy is available
                if std::process::Command::new("which")
                    .arg("icalBuddy")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                {
                    lines.insert(0, "- **Calendar/일정 조회** (preferred) → `exec` with `icalBuddy`. Example: command=\"icalBuddy\" args=[\"eventsFrom:today\", \"to:today+7\"]".to_string());
                }
                lines.push("- **macOS apps** → `exec` with `osascript` for Reminders, Notes, Contacts, etc. Each -e flag = one AppleScript line.".to_string());
                lines.push("- **System info** → `exec` with `sw_vers`, `sysctl`, `diskutil`, etc.".to_string());
                lines.join("\n")
            }
            "linux" => {
                "- **Calendar** → `exec` with `calcurse` or `gcalcli` if available\n\
                 - **System info** → `exec` with `uname -a`, `lsb_release -a`, `df -h`, etc."
                    .to_string()
            }
            _ => String::new(),
        };

        // Discover common project directories
        let machine_extra = {
            let mut extras = Vec::new();
            let candidate_dirs = [
                format!("{}/projects", home_dir),
                format!("{}/Documents", home_dir),
                format!("{}/Desktop", home_dir),
                "/Volumes".to_string(),
            ];
            for dir in &candidate_dirs {
                if std::path::Path::new(dir).is_dir() {
                    extras.push(format!("- Known directory: `{}`", dir));
                }
            }
            extras.join("\n")
        };

        self.system_prompt = self
            .system_prompt
            .replace("{username}", &username)
            .replace("{home_dir}", &home_dir)
            .replace("{os_type}", os_type)
            .replace("{machine_extra}", &machine_extra)
            .replace("{local_app_instructions}", &local_app_instructions);
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

    /// Get the underlying LLM provider
    #[must_use]
    pub fn provider(&self) -> &dyn LlmProvider {
        self.provider.as_ref()
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

    /// Lightweight classification — no tools, low max_tokens, temperature=0
    ///
    /// Uses 128 output tokens to accommodate Gemini 2.5's internal thinking
    /// overhead. The model only needs ~1-2 tokens for the persona name, but
    /// thinking tokens consume the same budget.
    pub async fn classify(&self, system_prompt: &str, user_input: &str) -> Result<String> {
        let model = self
            .config
            .default_model
            .clone()
            .unwrap_or_else(|| self.provider.default_model().to_string());
        let request = CompletionRequest {
            model,
            messages: vec![Message::system(system_prompt), Message::user(user_input)],
            max_tokens: Some(128),
            temperature: Some(0.0),
            stop: None,
        };
        let response = self.provider.complete(request).await.map_err(Error::Llm)?;
        Ok(response.content.trim().to_lowercase())
    }

    /// Plan a single step with the given messages and tools
    #[instrument(skip(self, messages, tools))]
    pub async fn plan_step(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<PlanResponse> {
        let mut full_messages = vec![Message::system(&self.config.system_prompt)];
        full_messages.extend(messages.iter().cloned());
        self.plan_step_impl(full_messages, tools).await
    }

    /// Plan a single step with a custom system prompt override
    #[instrument(skip(self, messages, tools, system_prompt))]
    pub async fn plan_step_with_system_prompt(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt: &str,
    ) -> Result<PlanResponse> {
        let mut full_messages = vec![Message::system(system_prompt)];
        full_messages.extend(messages.iter().cloned());
        self.plan_step_impl(full_messages, tools).await
    }

    /// Common planning implementation
    async fn plan_step_impl(
        &self,
        full_messages: Vec<Message>,
        tools: &[ToolDefinition],
    ) -> Result<PlanResponse> {
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

            let llm_start = std::time::Instant::now();
            let response = self.provider.complete(request).await.map_err(Error::Llm)?;
            let llm_secs = llm_start.elapsed().as_secs_f64();

            // Record LLM metrics
            let provider_name = self.provider.name();
            crate::utils::metrics_global::labeled_counter("cratos_llm_requests_total")
                .inc(&[("provider", provider_name), ("model", &response.model)]);
            crate::utils::metrics_global::labeled_histogram("cratos_llm_duration_seconds")
                .observe(&[("provider", provider_name)], llm_secs);
            if let Some(ref usage) = response.usage {
                crate::utils::metrics_global::labeled_counter("cratos_llm_tokens_total")
                    .inc_by(
                        &[("provider", provider_name), ("direction", "input")],
                        u64::from(usage.prompt_tokens),
                    );
                crate::utils::metrics_global::labeled_counter("cratos_llm_tokens_total")
                    .inc_by(
                        &[("provider", provider_name), ("direction", "output")],
                        u64::from(usage.completion_tokens),
                    );
            }

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

            let llm_start = std::time::Instant::now();
            let response = self
                .provider
                .complete_with_tools(request)
                .await
                .map_err(Error::Llm)?;
            let llm_secs = llm_start.elapsed().as_secs_f64();

            // Record LLM metrics
            let provider_name = self.provider.name();
            crate::utils::metrics_global::labeled_counter("cratos_llm_requests_total")
                .inc(&[("provider", provider_name), ("model", &response.model)]);
            crate::utils::metrics_global::labeled_histogram("cratos_llm_duration_seconds")
                .observe(&[("provider", provider_name)], llm_secs);
            if let Some(ref usage) = response.usage {
                crate::utils::metrics_global::labeled_counter("cratos_llm_tokens_total")
                    .inc_by(
                        &[("provider", provider_name), ("direction", "input")],
                        u64::from(usage.prompt_tokens),
                    );
                crate::utils::metrics_global::labeled_counter("cratos_llm_tokens_total")
                    .inc_by(
                        &[("provider", provider_name), ("direction", "output")],
                        u64::from(usage.completion_tokens),
                    );
            }

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
    /// Too low → useful data (calendar events, file contents) gets truncated
    /// and the model may report "no results". Too high → Gemini loops on huge
    /// outputs (e.g. `ps aux`). 4000 balances both concerns.
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
            thought_signature: None,
        }];
        let results = vec![serde_json::json!({"result": "ok"})];

        let messages = Planner::build_tool_result_messages(&calls, &results);
        assert_eq!(messages.len(), 1);
    }
}
