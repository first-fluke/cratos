//! Server configuration types
//!
//! Contains all configuration structures for the Cratos server.

use crate::middleware::rate_limit::RateLimitSettings;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct AppConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub data_dir: Option<String>,
    pub redis: RedisConfig,
    pub llm: LlmConfig,
    pub approval: ApprovalConfig,
    pub replay: ReplayConfig,
    pub channels: ChannelsConfig,
    #[serde(default)]
    pub vector_search: VectorSearchConfig,
    #[serde(default)]
    pub scheduler: SchedulerAppConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub canvas: CanvasConfig,
    #[serde(default)]
    pub orchestrator: OrchestratorAppConfig,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_persona")]
    pub persona: String,
}

fn default_language() -> String {
    "auto".to_string()
}

fn default_persona() -> String {
    "cratos".to_string()
}

impl AppConfig {
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }
        fs::write(path, content).context("Failed to write config file")?;
        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            data_dir: None,
            redis: RedisConfig::default(),
            llm: LlmConfig::default(),
            approval: ApprovalConfig::default(),
            replay: ReplayConfig::default(),
            channels: ChannelsConfig::default(),
            vector_search: VectorSearchConfig::default(),
            scheduler: SchedulerAppConfig::default(),
            security: SecurityConfig::default(),
            canvas: CanvasConfig::default(),
            orchestrator: OrchestratorAppConfig::default(),
            language: default_language(),
            persona: default_persona(),
        }
    }
}

/// Orchestrator configuration (exposed to TOML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorAppConfig {
    /// Maximum LLM iterations per request (tool-calling loop)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    /// Maximum total execution time in seconds (0 = no limit)
    #[serde(default = "default_max_execution_secs")]
    pub max_execution_secs: u64,
    /// Bail out after this many consecutive all-fail iterations
    #[serde(default = "default_max_consecutive_failures")]
    pub max_consecutive_failures: usize,
    /// Bail out after this many total tool failures
    #[serde(default = "default_max_total_failures")]
    pub max_total_failures: usize,
    /// Enable automatic skill pattern detection
    #[serde(default = "default_true")]
    pub auto_skill_detection: bool,
}

impl Default for OrchestratorAppConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            max_execution_secs: default_max_execution_secs(),
            max_consecutive_failures: default_max_consecutive_failures(),
            max_total_failures: default_max_total_failures(),
            auto_skill_detection: true,
        }
    }
}

fn default_max_iterations() -> usize {
    20
}
fn default_max_execution_secs() -> u64 {
    180
}
fn default_max_consecutive_failures() -> usize {
    3
}
fn default_max_total_failures() -> usize {
    6
}

/// Scheduler configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]

pub struct SchedulerAppConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_secs: u64,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    #[serde(default = "default_true")]
    pub logging_enabled: bool,
}

fn default_check_interval() -> u64 {
    60
}

fn default_retry_delay() -> u64 {
    30
}

fn default_max_concurrent() -> usize {
    10
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub rate_limit: RateLimitSettings,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct AuthConfig {
    /// Enable authentication
    #[serde(default)]
    pub enabled: bool,
    /// Auto-generate admin API key on first run
    #[serde(default = "default_true")]
    pub auto_generate_key: bool,
    /// Key storage backend: keychain | encrypted_file | memory
    #[serde(default = "default_key_storage")]
    pub key_storage: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_generate_key: true,
            key_storage: "keychain".to_string(),
        }
    }
}

fn default_key_storage() -> String {
    "keychain".to_string()
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

/// LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct LlmConfig {
    pub default_provider: String,
    pub openai: Option<OpenAiLlmConfig>,
    pub anthropic: Option<AnthropicLlmConfig>,
    pub gemini: Option<GeminiLlmConfig>,
    pub routing: Option<RoutingConfig>,
    /// Model routing configuration for cost-optimized tiered routing
    #[serde(default)]
    pub model_routing: Option<ModelRoutingConfig>,
}

/// Model routing configuration loaded from [llm.model_routing] in TOML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoutingConfig {
    pub simple: Option<RouteEntry>,
    pub general: Option<RouteEntry>,
    pub complex: Option<RouteEntry>,
    pub fallback: Option<RouteEntry>,
    #[serde(default)]
    pub auto_downgrade: Option<bool>,
}

/// A single route entry (provider + model pair)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEntry {
    pub provider: String,
    pub model: String,
}

/// OpenAI-specific config
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct OpenAiLlmConfig {
    pub default_model: String,
}

/// Anthropic-specific config
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct AnthropicLlmConfig {
    pub default_model: String,
}

/// Gemini-specific config
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct GeminiLlmConfig {
    pub default_model: String,
}

/// Routing configuration for model selection
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct RoutingConfig {
    pub classification: Option<String>,
    pub planning: Option<String>,
    pub code_generation: Option<String>,
    pub summarization: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 19527,
            auth: AuthConfig::default(),
            rate_limit: RateLimitSettings::default(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            default_provider: "auto".to_string(),
            openai: None,
            anthropic: None,
            gemini: None,
            routing: None,
            model_routing: None,
        }
    }
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            default_mode: "never".to_string(),
        }
    }
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            retention_days: 30,
            max_events_per_execution: 1000,
        }
    }
}

impl Default for ChannelsConfig {
    fn default() -> Self {
        Self {
            telegram: TelegramChannelConfig { enabled: false },
            slack: SlackChannelConfig { enabled: false },
            discord: DiscordChannelConfig::default(),
            matrix: MatrixChannelConfig::default(),
            whatsapp: WhatsAppChannelConfig::default(),
            whatsapp_business: WhatsAppBusinessChannelConfig::default(),
        }
    }
}

/// Approval configuration
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct ApprovalConfig {
    pub default_mode: String,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]

pub struct SecurityConfig {
    #[serde(default)]
    pub exec: ExecSecurityConfig,
    #[serde(default)]
    pub sandbox_policy: Option<String>,
    #[serde(default)]
    pub credential_backend: Option<String>,
    #[serde(default)]
    pub enable_injection_protection: Option<bool>,
}

/// Exec security configuration (from [security.exec] in TOML)
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct ExecSecurityConfig {
    /// "permissive" (default) or "strict"
    #[serde(default = "default_exec_mode")]
    pub mode: String,
    /// Maximum timeout in seconds
    #[serde(default = "default_exec_timeout")]
    pub max_timeout_secs: u64,
    /// Additional commands to block
    #[serde(default)]
    pub extra_blocked_commands: Vec<String>,
    /// Allowed commands (only used when mode = "strict")
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    /// Blocked filesystem paths
    #[serde(default = "default_blocked_paths")]
    pub blocked_paths: Vec<String>,
}

impl Default for ExecSecurityConfig {
    fn default() -> Self {
        Self {
            mode: "permissive".to_string(),
            max_timeout_secs: 60,
            extra_blocked_commands: Vec::new(),
            allowed_commands: Vec::new(),
            blocked_paths: default_blocked_paths(),
        }
    }
}

fn default_exec_mode() -> String {
    "permissive".to_string()
}

fn default_exec_timeout() -> u64 {
    120
}

fn default_blocked_paths() -> Vec<String> {
    vec![
        "/etc".to_string(),
        "/root".to_string(),
        "/var/log".to_string(),
        "/boot".to_string(),
        "/dev".to_string(),
        "/proc".to_string(),
        "/sys".to_string(),
        "/usr/bin".to_string(),
        "/usr/sbin".to_string(),
        "/bin".to_string(),
        "/sbin".to_string(),
    ]
}

/// Replay configuration
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct ReplayConfig {
    pub retention_days: u32,
    pub max_events_per_execution: u32,
}

/// Channels configuration
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct ChannelsConfig {
    pub telegram: TelegramChannelConfig,
    pub slack: SlackChannelConfig,
    #[serde(default)]
    pub discord: DiscordChannelConfig,
    #[serde(default)]
    pub matrix: MatrixChannelConfig,
    #[serde(default)]
    pub whatsapp: WhatsAppChannelConfig,
    #[serde(default)]
    pub whatsapp_business: WhatsAppBusinessChannelConfig,
}

/// Telegram channel config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramChannelConfig {
    pub enabled: bool,
}

/// Slack channel config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackChannelConfig {
    pub enabled: bool,
}

/// Discord channel config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]

pub struct DiscordChannelConfig {
    #[serde(default)]
    pub enabled: bool,
}

/// Matrix channel config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]

pub struct MatrixChannelConfig {
    #[serde(default)]
    pub enabled: bool,
}

/// WhatsApp (Baileys) channel config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]

pub struct WhatsAppChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_whatsapp_bridge_url")]
    pub bridge_url: String,
}

fn default_whatsapp_bridge_url() -> String {
    "http://localhost:3001".to_string()
}

/// WhatsApp Business API channel config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]

pub struct WhatsAppBusinessChannelConfig {
    #[serde(default)]
    pub enabled: bool,
}

/// Vector search configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]

pub struct VectorSearchConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_dimensions")]
    pub dimensions: usize,
}

/// Canvas (live document editing) configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]

pub struct CanvasConfig {
    /// Enable canvas feature
    #[serde(default)]
    pub enabled: bool,
    /// Maximum concurrent editing sessions
    #[serde(default = "default_max_canvas_sessions")]
    pub max_sessions: usize,
}

fn default_max_canvas_sessions() -> usize {
    100
}

pub(crate) fn default_true() -> bool {
    true
}

fn default_dimensions() -> usize {
    768
}

// ============================================================================
// Shared Configuration Validator
// ============================================================================

/// Shared validation logic for CLI, API, and TUI
pub struct ConfigValidator;

impl ConfigValidator {
    /// Known LLM provider names
    const VALID_PROVIDERS: &[&str] = &[
        "openai",
        "anthropic",
        "gemini",
        "google",
        "google_pro",
        "groq",
        "deepseek",
        "ollama",
        "openrouter",
        "novita",
        "glm",
        "moonshot",
        "qwen",
        "auto",
    ];

    const VALID_LANGUAGES: &[&str] = &["en", "ko", "ja", "zh", "auto"];

    const VALID_PERSONAS: &[&str] = &[
        "cratos", "athena", "sindri", "heimdall", "mimir", "odin", "hestia", "norns", "apollo",
        "freya", "tyr", "nike", "thor", "brok",
    ];

    const VALID_APPROVAL_MODES: &[&str] = &["always", "risky_only", "never"];

    const VALID_EXEC_MODES: &[&str] = &["permissive", "strict"];

    pub fn validate_provider(s: &str) -> Result<(), String> {
        if Self::VALID_PROVIDERS.contains(&s) {
            Ok(())
        } else {
            Err(format!(
                "Invalid provider '{}'. Valid: {}",
                s,
                Self::VALID_PROVIDERS.join(", ")
            ))
        }
    }

    pub fn validate_language(s: &str) -> Result<(), String> {
        if Self::VALID_LANGUAGES.contains(&s) {
            Ok(())
        } else {
            Err(format!(
                "Invalid language '{}'. Valid: {}",
                s,
                Self::VALID_LANGUAGES.join(", ")
            ))
        }
    }

    pub fn validate_persona(s: &str) -> Result<(), String> {
        if Self::VALID_PERSONAS.contains(&s) {
            Ok(())
        } else {
            Err(format!(
                "Invalid persona '{}'. Valid: {}",
                s,
                Self::VALID_PERSONAS.join(", ")
            ))
        }
    }

    pub fn validate_approval_mode(s: &str) -> Result<(), String> {
        if Self::VALID_APPROVAL_MODES.contains(&s) {
            Ok(())
        } else {
            Err(format!(
                "Invalid approval mode '{}'. Valid: {}",
                s,
                Self::VALID_APPROVAL_MODES.join(", ")
            ))
        }
    }

    pub fn validate_exec_mode(s: &str) -> Result<(), String> {
        if Self::VALID_EXEC_MODES.contains(&s) {
            Ok(())
        } else {
            Err(format!(
                "Invalid exec mode '{}'. Valid: {}",
                s,
                Self::VALID_EXEC_MODES.join(", ")
            ))
        }
    }

    pub fn validate_port(p: u16) -> Result<(), String> {
        if p == 0 {
            Err("Port cannot be 0".to_string())
        } else {
            Ok(())
        }
    }
}
