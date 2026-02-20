//! CLI Provider Registry
//!
//! Maps agents to their preferred AI CLI providers.

use async_trait::async_trait;
use cratos_llm::{CompletionRequest, LlmRouter, Message};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::process::Command;
use tracing::debug;

/// CLI-related errors
#[derive(Debug, Error)]
pub enum CliError {
    /// Provider not found
    #[error("CLI provider '{0}' not found")]
    ProviderNotFound(String),

    /// Execution failed
    #[error("CLI execution failed: {0}")]
    ExecutionFailed(String),

    /// Timeout
    #[error("CLI execution timed out")]
    Timeout,

    /// Configuration error
    #[error("CLI configuration error: {0}")]
    Configuration(String),
}

/// CLI result type
pub type CliResult<T> = std::result::Result<T, CliError>;

/// CLI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    /// Provider name
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Default arguments
    #[serde(default)]
    pub default_args: Vec<String>,
    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Default timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    300
}

impl CliConfig {
    /// Create a new CLI config
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            default_args: Vec::new(),
            env: HashMap::new(),
            timeout_seconds: default_timeout(),
        }
    }

    /// Create Claude CLI config
    pub fn claude() -> Self {
        Self {
            name: "claude".to_string(),
            command: "claude".to_string(),
            default_args: vec!["--print".to_string()],
            env: HashMap::new(),
            timeout_seconds: 300,
        }
    }

    /// Create Gemini CLI config
    pub fn gemini() -> Self {
        Self {
            name: "gemini".to_string(),
            command: "gemini".to_string(),
            default_args: vec![],
            env: HashMap::new(),
            timeout_seconds: 180,
        }
    }

    /// Create Groq CLI config (via API)
    pub fn groq() -> Self {
        Self {
            name: "groq".to_string(),
            command: "cratos-cli".to_string(), // Internal CLI that uses Groq API
            default_args: vec!["--provider".to_string(), "groq".to_string()],
            env: HashMap::new(),
            timeout_seconds: 120,
        }
    }
}

/// CLI provider trait
#[async_trait]
pub trait CliProvider: Send + Sync {
    /// Get provider name
    fn name(&self) -> &str;

    /// Execute a prompt
    async fn execute(
        &self,
        prompt: &str,
        persona: &str,
        workspace: Option<&Path>,
    ) -> CliResult<String>;

    /// Check if provider is available
    async fn is_available(&self) -> bool;
}

/// Default CLI provider implementation (shell-based)
pub struct ShellCliProvider {
    config: CliConfig,
}

impl ShellCliProvider {
    /// Create a new shell CLI provider
    pub fn new(config: CliConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl CliProvider for ShellCliProvider {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn execute(
        &self,
        prompt: &str,
        persona: &str,
        workspace: Option<&Path>,
    ) -> CliResult<String> {
        // Build the full prompt with persona
        let full_prompt = if persona.is_empty() {
            prompt.to_string()
        } else {
            format!("{}\n\nTask: {}", persona, prompt)
        };

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.default_args);
        cmd.arg(&full_prompt);

        // Set environment variables
        for (key, value) in &self.config.env {
            // Expand ${VAR} references
            let expanded = if value.starts_with("${") && value.ends_with('}') {
                let var_name = &value[2..value.len() - 1];
                std::env::var(var_name).unwrap_or_default()
            } else {
                value.clone()
            };
            cmd.env(key, expanded);
        }

        // Set working directory if specified
        if let Some(workspace) = workspace {
            cmd.current_dir(workspace);
        }

        debug!(provider = %self.config.name, command = %self.config.command, "Executing CLI");

        let timeout = Duration::from_secs(self.config.timeout_seconds);
        let output = tokio::time::timeout(timeout, cmd.output())
            .await
            .map_err(|_| CliError::Timeout)?
            .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CliError::ExecutionFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.to_string())
    }

    async fn is_available(&self) -> bool {
        Command::new("which")
            .arg(&self.config.command)
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// API-based CLI provider (routes through LlmRouter)
pub struct ApiCliProvider {
    name: String,
    provider: String,
    model: Option<String>,
    router: Option<Arc<LlmRouter>>,
}

impl ApiCliProvider {
    /// Create a new API CLI provider
    pub fn new(
        name: impl Into<String>,
        provider: impl Into<String>,
        model: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            provider: provider.into(),
            model,
            router: None,
        }
    }

    /// Create with LLM router for direct API calls
    pub fn with_router(
        name: impl Into<String>,
        provider: impl Into<String>,
        model: Option<String>,
        router: Arc<LlmRouter>,
    ) -> Self {
        Self {
            name: name.into(),
            provider: provider.into(),
            model,
            router: Some(router),
        }
    }

    /// Build messages from prompt and persona
    fn build_messages(prompt: &str, persona: &str) -> Vec<Message> {
        let mut messages = Vec::new();
        if !persona.is_empty() {
            messages.push(Message::system(persona));
        }
        messages.push(Message::user(prompt));
        messages
    }
}

#[async_trait]
impl CliProvider for ApiCliProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(
        &self,
        prompt: &str,
        persona: &str,
        _workspace: Option<&Path>,
    ) -> CliResult<String> {
        let Some(router) = &self.router else {
            return Err(CliError::Configuration(
                "LLM router not configured for API provider".to_string(),
            ));
        };

        let messages = Self::build_messages(prompt, persona);
        let model = self.model.clone().unwrap_or_default();

        let request = CompletionRequest {
            model,
            messages,
            max_tokens: Some(4096),
            temperature: Some(0.7),
            stop: None,
        };

        let response = router
            .complete_with(&self.provider, request)
            .await
            .map_err(|e| CliError::ExecutionFailed(e.to_string()))?;

        Ok(response.content)
    }

    async fn is_available(&self) -> bool {
        let key_name = match self.provider.as_str() {
            "groq" => "GROQ_API_KEY",
            "deepseek" => "DEEPSEEK_API_KEY",
            "anthropic" => "ANTHROPIC_API_KEY",
            "openai" => "OPENAI_API_KEY",
            _ => return false,
        };
        std::env::var(key_name).is_ok()
    }
}

/// Registry of CLI providers
pub struct CliRegistry {
    providers: HashMap<String, Box<dyn CliProvider>>,
}

impl CliRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Create with default providers (no LLM router — API providers will error on execute)
    pub fn with_defaults() -> Self {
        Self::with_router(None)
    }

    /// Create with default providers and an optional LLM router
    pub fn with_router(router: Option<Arc<LlmRouter>>) -> Self {
        let mut registry = Self::new();

        // Register shell-based providers
        registry.register(Box::new(ShellCliProvider::new(CliConfig::claude())));
        registry.register(Box::new(ShellCliProvider::new(CliConfig::gemini())));

        // Register API-based providers (connected to LLM router if available)
        let api_providers = [
            ("groq", "groq"),
            ("deepseek", "deepseek"),
            ("anthropic", "anthropic"),
            ("openai", "openai"),
        ];
        for (name, provider) in api_providers {
            let cli_provider: Box<dyn CliProvider> = match &router {
                Some(r) => Box::new(ApiCliProvider::with_router(
                    name,
                    provider,
                    None,
                    Arc::clone(r),
                )),
                None => Box::new(ApiCliProvider::new(name, provider, None)),
            };
            registry.register(cli_provider);
        }

        registry
    }

    /// Register a provider
    pub fn register(&mut self, provider: Box<dyn CliProvider>) {
        let name = provider.name().to_string();
        debug!(provider = %name, "Registering CLI provider");
        self.providers.insert(name, provider);
    }

    /// Get a provider by name
    pub fn get(&self, name: &str) -> Option<&dyn CliProvider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    /// List all provider names
    pub fn list(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Check which providers are available
    pub async fn available_providers(&self) -> Vec<&str> {
        let mut available = Vec::new();
        for (name, provider) in &self.providers {
            if provider.is_available().await {
                available.push(name.as_str());
            }
        }
        available
    }
}

impl Default for CliRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests;

