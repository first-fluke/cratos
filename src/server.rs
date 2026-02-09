//! Server module for Cratos
//!
//! Contains the main server initialization and runtime logic.

use anyhow::{Context, Result};
use axum::{routing::get, Extension, Json, Router};
use config::{Config, Environment, File, FileFormat};
use cratos_channels::{TelegramAdapter, TelegramConfig};
use cratos_core::{
    admin_scopes, metrics_global, shutdown_signal_with_controller,
    ApprovalManager, AuthStore, EventBus, OlympusConfig, OlympusHooks, Orchestrator,
    OrchestratorConfig, PlannerConfig, RedisStore, SchedulerConfig, SchedulerEngine,
    SchedulerStore, SessionStore, ShutdownController,
};
use crate::middleware::rate_limit::{RateLimitLayer, RateLimitSettings};
use cratos_llm::{
    AnthropicConfig, AnthropicProvider, DeepSeekConfig, DeepSeekProvider, EmbeddingProvider,
    GeminiConfig, GeminiProvider, GlmConfig, GlmProvider, GroqConfig, GroqProvider, LlmProvider,
    LlmRouter, MoonshotConfig, MoonshotProvider, NovitaConfig, NovitaProvider, OllamaConfig,
    OllamaProvider, OpenAiConfig, OpenAiProvider, OpenRouterConfig, OpenRouterProvider, QwenConfig,
    QwenProvider, SharedEmbeddingProvider, TractEmbeddingProvider,
};
use cratos_replay::{EventStore, ExecutionSearcher, SearchEmbedder};
use cratos_memory::{GraphMemory, VectorBridge};
use cratos_search::{IndexConfig, VectorIndex};
use cratos_skills::{SemanticSkillRouter, SkillEmbedder, SkillRegistry, SkillStore};
use cratos_tools::{register_builtins_with_config, BuiltinsConfig, ExecConfig, ExecMode, RunnerConfig, ToolRegistry};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Application configuration
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
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
}

/// Scheduler configuration
#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
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
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub rate_limit: RateLimitSettings,
}

/// Authentication configuration
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
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
            enabled: false,
            auto_generate_key: true,
            key_storage: "keychain".to_string(),
        }
    }
}

fn default_key_storage() -> String {
    "keychain".to_string()
}

/// Redis configuration
#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

/// LLM configuration
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct LlmConfig {
    pub default_provider: String,
    pub openai: Option<OpenAiLlmConfig>,
    pub anthropic: Option<AnthropicLlmConfig>,
    pub routing: Option<RoutingConfig>,
}

/// OpenAI-specific config
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OpenAiLlmConfig {
    pub default_model: String,
}

/// Anthropic-specific config
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AnthropicLlmConfig {
    pub default_model: String,
}

/// Routing configuration for model selection
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct RoutingConfig {
    pub classification: Option<String>,
    pub planning: Option<String>,
    pub code_generation: Option<String>,
    pub summarization: Option<String>,
}

/// Approval configuration
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ApprovalConfig {
    pub default_mode: String,
}

/// Security configuration
#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
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
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
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
    60
}

fn default_blocked_paths() -> Vec<String> {
    vec![
        "/etc".to_string(), "/root".to_string(), "/var/log".to_string(),
        "/boot".to_string(), "/dev".to_string(), "/proc".to_string(),
        "/sys".to_string(), "/usr/bin".to_string(), "/usr/sbin".to_string(),
        "/bin".to_string(), "/sbin".to_string(),
    ]
}

/// Replay configuration
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ReplayConfig {
    pub retention_days: u32,
    pub max_events_per_execution: u32,
}

/// Channels configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelsConfig {
    pub telegram: TelegramChannelConfig,
    pub slack: SlackChannelConfig,
}

/// Telegram channel config
#[derive(Debug, Clone, Deserialize)]
pub struct TelegramChannelConfig {
    pub enabled: bool,
}

/// Slack channel config
#[derive(Debug, Clone, Deserialize)]
pub struct SlackChannelConfig {
    pub enabled: bool,
}

/// Vector search configuration
#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct VectorSearchConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_dimensions")]
    pub dimensions: usize,
}

fn default_true() -> bool {
    true
}

fn default_dimensions() -> usize {
    768
}

/// Adapter to use EmbeddingProvider as SearchEmbedder
struct EmbeddingAdapter {
    provider: SharedEmbeddingProvider,
}

#[async_trait::async_trait]
impl SearchEmbedder for EmbeddingAdapter {
    async fn embed(&self, text: &str) -> cratos_replay::Result<Vec<f32>> {
        self.provider
            .embed(text)
            .await
            .map_err(|e| cratos_replay::Error::Database(format!("Embedding failed: {}", e)))
    }

    async fn embed_batch(&self, texts: &[String]) -> cratos_replay::Result<Vec<Vec<f32>>> {
        self.provider
            .embed_batch(texts)
            .await
            .map_err(|e| cratos_replay::Error::Database(format!("Batch embedding failed: {}", e)))
    }

    fn dimensions(&self) -> usize {
        self.provider.dimensions()
    }
}

/// Adapter to use EmbeddingProvider as SkillEmbedder
struct SkillEmbeddingAdapter {
    provider: SharedEmbeddingProvider,
}

#[async_trait::async_trait]
impl SkillEmbedder for SkillEmbeddingAdapter {
    async fn embed(&self, text: &str) -> cratos_skills::Result<Vec<f32>> {
        self.provider
            .embed(text)
            .await
            .map_err(|e| cratos_skills::Error::Internal(format!("Embedding failed: {}", e)))
    }

    async fn embed_batch(&self, texts: &[String]) -> cratos_skills::Result<Vec<Vec<f32>>> {
        self.provider
            .embed_batch(texts)
            .await
            .map_err(|e| cratos_skills::Error::Internal(format!("Batch embedding failed: {}", e)))
    }

    fn dimensions(&self) -> usize {
        self.provider.dimensions()
    }
}

/// Health check response
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

/// Detailed health check response
#[derive(Debug, Serialize)]
struct DetailedHealthResponse {
    status: &'static str,
    version: &'static str,
    checks: HealthChecks,
}

/// Individual health checks
#[derive(Debug, Serialize)]
struct HealthChecks {
    database: ComponentHealth,
    redis: ComponentHealth,
}

/// Component health status
#[derive(Debug, Serialize)]
struct ComponentHealth {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ComponentHealth {
    fn healthy(latency_ms: u64) -> Self {
        Self {
            status: "healthy",
            latency_ms: Some(latency_ms),
            error: None,
        }
    }

    fn unhealthy(error: String) -> Self {
        Self {
            status: "unhealthy",
            latency_ms: None,
            error: Some(error),
        }
    }
}

/// Simple health check endpoint (for load balancers)
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Detailed health check with component status
async fn detailed_health_check(
    redis_url: axum::extract::Extension<String>,
) -> Json<DetailedHealthResponse> {
    let db_health = ComponentHealth::healthy(0);

    let redis_health = {
        let start = std::time::Instant::now();
        match redis::Client::open(redis_url.as_str()) {
            Ok(client) => match client.get_multiplexed_async_connection().await {
                Ok(mut conn) => match redis::cmd("PING").query_async::<String>(&mut conn).await {
                    Ok(_) => ComponentHealth::healthy(start.elapsed().as_millis() as u64),
                    Err(e) => ComponentHealth::unhealthy(e.to_string()),
                },
                Err(e) => ComponentHealth::unhealthy(e.to_string()),
            },
            Err(e) => ComponentHealth::unhealthy(e.to_string()),
        }
    };

    let overall_status = if db_health.status == "healthy" && redis_health.status == "healthy" {
        "healthy"
    } else if db_health.status == "healthy" || redis_health.status == "healthy" {
        "degraded"
    } else {
        "unhealthy"
    };

    Json(DetailedHealthResponse {
        status: overall_status,
        version: env!("CARGO_PKG_VERSION"),
        checks: HealthChecks {
            database: db_health,
            redis: redis_health,
        },
    })
}

/// Metrics endpoint (Prometheus format)
async fn metrics_endpoint() -> String {
    metrics_global::export_prometheus()
}

/// Embedded default configuration (compiled into binary)
const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");

/// Load configuration from files and environment
pub(crate) fn load_config() -> Result<AppConfig> {
    let config = Config::builder()
        // 1. Embedded defaults (always available)
        .add_source(File::from_str(DEFAULT_CONFIG, FileFormat::Toml))
        // 2. External overrides (optional)
        .add_source(File::with_name("config/default").required(false))
        .add_source(File::with_name("config/local").required(false))
        .add_source(
            File::with_name(&format!(
                "config/{}",
                std::env::var("CRATOS_ENV").unwrap_or_else(|_| "development".to_string())
            ))
            .required(false),
        )
        // 3. Environment variables (highest priority)
        // prefix_separator("_") ensures CRATOS_LLM__X works (single _ after prefix).
        // Without it, config-rs 0.14 defaults prefix_separator to separator ("__"),
        // requiring CRATOS__LLM__X which doesn't match .env convention.
        .add_source(
            Environment::with_prefix("CRATOS")
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true),
        )
        .build()
        .context("Failed to build configuration")?;

    config
        .try_deserialize()
        .context("Failed to deserialize configuration")
}

pub(crate) fn resolve_llm_provider(llm_config: &LlmConfig) -> Result<Arc<LlmRouter>> {
    let mut router = LlmRouter::new(&llm_config.default_provider);
    let mut registered_count = 0;
    let mut default_provider: Option<String> = None;

    if let Ok(config) = GroqConfig::from_env() {
        if let Ok(provider) = GroqProvider::new(config) {
            router.register("groq", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("groq".to_string());
            }
            info!("Registered Groq provider");
        }
    }
    if let Ok(config) = OpenRouterConfig::from_env() {
        if let Ok(provider) = OpenRouterProvider::new(config) {
            router.register("openrouter", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("openrouter".to_string());
            }
            info!("Registered OpenRouter provider");
        }
    }
    if let Ok(config) = NovitaConfig::from_env() {
        if let Ok(provider) = NovitaProvider::new(config) {
            router.register("novita", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("novita".to_string());
            }
            info!("Registered Novita provider (free tier)");
        }
    }
    if let Ok(config) = DeepSeekConfig::from_env() {
        if let Ok(provider) = DeepSeekProvider::new(config) {
            router.register("deepseek", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("deepseek".to_string());
            }
            info!("Registered DeepSeek provider (low cost)");
        }
    }
    if let Ok(config) = OpenAiConfig::from_env() {
        let auth_source = config.auth_source;
        cratos_llm::cli_auth::register_auth_source("openai", auth_source);
        let provider = OpenAiProvider::new(config);
        router.register("openai", Arc::new(provider));
        registered_count += 1;
        if default_provider.is_none() {
            default_provider = Some("openai".to_string());
        }
        info!("Registered OpenAI provider ({})", auth_source);
    }
    if let Ok(config) = AnthropicConfig::from_env() {
        if let Ok(provider) = AnthropicProvider::new(config) {
            router.register("anthropic", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("anthropic".to_string());
            }
            info!("Registered Anthropic provider");
        }
    }
    match GeminiConfig::from_env() {
        Ok(config) => {
            let auth_source = config.auth_source;
            cratos_llm::cli_auth::register_auth_source("gemini", auth_source);
            match GeminiProvider::new(config) {
                Ok(provider) => {
                    router.register("gemini", Arc::new(provider));
                    registered_count += 1;
                    if default_provider.is_none() {
                        default_provider = Some("gemini".to_string());
                    }
                    info!("Registered Gemini provider ({})", auth_source);
                }
                Err(e) => debug!("Gemini provider init failed: {}", e),
            }
        }
        Err(e) => debug!("Gemini config not available: {}", e),
    }
    if let Ok(config) = GlmConfig::from_env() {
        if let Ok(provider) = GlmProvider::new(config) {
            router.register("glm", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("glm".to_string());
            }
            info!("Registered GLM provider");
        }
    }
    if let Ok(config) = MoonshotConfig::from_env() {
        if let Ok(provider) = MoonshotProvider::new(config) {
            router.register("moonshot", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("moonshot".to_string());
            }
            info!("Registered Moonshot provider");
        }
    }
    if let Ok(config) = QwenConfig::from_env() {
        if let Ok(provider) = QwenProvider::new(config) {
            router.register("qwen", Arc::new(provider));
            registered_count += 1;
            if default_provider.is_none() {
                default_provider = Some("qwen".to_string());
            }
            info!("Registered Qwen provider");
        }
    }
    let ollama_config = OllamaConfig::from_env();
    if let Ok(provider) = OllamaProvider::new(ollama_config) {
        router.register("ollama", Arc::new(provider));
        registered_count += 1;
        if default_provider.is_none() {
            default_provider = Some("ollama".to_string());
        }
        info!("Registered Ollama provider (local)");
    }

    if registered_count == 0 {
        return Err(anyhow::anyhow!(
            "No LLM provider configured.\n\n\
             To fix this, run one of:\n\
               cratos init     # Interactive setup wizard (recommended)\n\
               cratos doctor   # Check your configuration\n\n\
             Or manually set one of these environment variables:\n\
               GROQ_API_KEY        # Free tier, recommended\n\
               OPENROUTER_API_KEY  # Free tier available\n\
               NOVITA_API_KEY      # Free tier\n\
               DEEPSEEK_API_KEY    # Ultra low cost\n\
               MOONSHOT_API_KEY    # Kimi K2\n\
               ZHIPU_API_KEY       # GLM-4.7\n\
               OPENAI_API_KEY\n\
               ANTHROPIC_API_KEY\n\n\
             Or use CLI subscription tokens:\n\
               gemini auth login   # Gemini CLI (Antigravity Pro)\n\
               codex auth login    # Codex CLI (ChatGPT Pro/Plus)"
        ));
    }

    // Normalize provider aliases (e.g., "google" -> "gemini")
    let normalized_provider = match llm_config.default_provider.as_str() {
        "google" => "gemini".to_string(),
        "zhipu" | "zhipuai" => "glm".to_string(),
        other => other.to_string(),
    };

    if normalized_provider == "auto" || normalized_provider.is_empty() {
        if let Some(dp) = default_provider {
            router.set_default(&dp);
            info!("Auto-selected default provider: {}", dp);
        } else if let Some(first) = router.list_providers().first() {
            let first = first.to_string();
            router.set_default(&first);
            info!("Auto-selected fallback provider: {}", first);
        }
    } else if router.has_provider(&normalized_provider) {
        router.set_default(&normalized_provider);
    } else {
        warn!(
            "Configured default provider '{}' not available, using auto-detected",
            normalized_provider
        );
        if let Some(dp) = default_provider {
            router.set_default(&dp);
        }
    }

    info!(
        "LLM Router initialized with {} providers: {:?}",
        registered_count,
        router.list_providers()
    );

    Ok(Arc::new(router))
}

/// Validate configuration for production security
fn validate_production_config(config: &AppConfig) -> Result<()> {
    let is_production = std::env::var("CRATOS_ENV")
        .map(|v| v.to_lowercase() == "production")
        .unwrap_or(false);

    if !is_production {
        return Ok(());
    }

    if config.server.host == "0.0.0.0" {
        warn!(
            "SECURITY WARNING: Server is binding to all interfaces (0.0.0.0) in production. \
             Consider binding to 127.0.0.1 and using a reverse proxy."
        );
    }

    if !config.server.auth.enabled && config.server.host != "127.0.0.1" {
        warn!(
            "SECURITY WARNING: Authentication is DISABLED while server is exposed externally. \
             Enable [server.auth] enabled = true in production!"
        );
    }

    if config.redis.url.starts_with("redis://") && !config.redis.url.contains('@') && is_production
    {
        warn!(
            "SECURITY WARNING: Redis connection appears to have no authentication in production. \
             Consider enabling Redis AUTH."
        );
    }

    let required_env_vars = ["TELEGRAM_BOT_TOKEN"];
    for var in required_env_vars {
        if config.channels.telegram.enabled && std::env::var(var).is_err() {
            warn!(
                "SECURITY WARNING: {} is not set but Telegram is enabled. \
                 Bot may not function correctly.",
                var
            );
        }
    }

    Ok(())
}

/// Run the server
pub async fn run() -> Result<()> {
    info!(
        "Starting Cratos AI Assistant v{}",
        env!("CARGO_PKG_VERSION")
    );

    let config = load_config().context("Failed to load configuration")?;
    info!("Configuration loaded");

    validate_production_config(&config)?;

    let data_dir = config
        .data_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(cratos_replay::default_data_dir);

    info!("Data directory: {}", data_dir.display());

    let db_path = data_dir.join("cratos.db");
    let event_store = Arc::new(
        EventStore::from_path(&db_path)
            .await
            .context("Failed to initialize SQLite event store")?,
    );
    info!("SQLite event store initialized at {}", db_path.display());

    let skill_db_path = data_dir.join("skills.db");
    let skill_store = Arc::new(
        SkillStore::from_path(&skill_db_path)
            .await
            .context("Failed to initialize SQLite skill store")?,
    );
    info!(
        "SQLite skill store initialized at {}",
        skill_db_path.display()
    );

    let skill_registry = Arc::new(SkillRegistry::new());
    let active_skills = skill_store.list_active_skills().await.unwrap_or_default();
    for skill in active_skills {
        if let Err(e) = skill_registry.register(skill).await {
            warn!("Failed to register skill: {}", e);
        }
    }
    let skill_count = skill_registry.count().await;
    info!(
        "Skill registry initialized with {} active skills",
        skill_count
    );

    let embedding_provider: Option<SharedEmbeddingProvider> = if config.vector_search.enabled {
        match TractEmbeddingProvider::new() {
            Ok(provider) => {
                info!(
                    "Embedding provider initialized: {} ({} dimensions)",
                    provider.name(),
                    provider.dimensions()
                );
                Some(Arc::new(provider))
            }
            Err(e) => {
                warn!(
                    "Failed to initialize embedding provider: {}. Semantic search disabled.",
                    e
                );
                None
            }
        }
    } else {
        info!("Vector search disabled by configuration");
        None
    };

    let vectors_dir = data_dir.join("vectors");
    let (_execution_searcher, _semantic_skill_router) = if let Some(ref embedder) =
        embedding_provider
    {
        std::fs::create_dir_all(&vectors_dir).context("Failed to create vectors directory")?;

        let dimensions = embedder.dimensions();

        let exec_index_path = vectors_dir.join("executions");
        let exec_index = match VectorIndex::open(&exec_index_path, IndexConfig::new(dimensions)) {
            Ok(idx) => {
                info!(
                    "Execution vector index loaded from {}",
                    exec_index_path.display()
                );
                idx
            }
            Err(e) => {
                warn!("Failed to load execution index, creating new: {}", e);
                VectorIndex::open(&exec_index_path, IndexConfig::new(dimensions))
                    .context("Failed to create execution vector index")?
            }
        };

        let skill_index_path = vectors_dir.join("skills");
        let skill_index = match VectorIndex::open(&skill_index_path, IndexConfig::new(dimensions)) {
            Ok(idx) => {
                info!(
                    "Skill vector index loaded from {}",
                    skill_index_path.display()
                );
                idx
            }
            Err(e) => {
                warn!("Failed to load skill index, creating new: {}", e);
                VectorIndex::open(&skill_index_path, IndexConfig::new(dimensions))
                    .context("Failed to create skill vector index")?
            }
        };

        let exec_embedder = Arc::new(EmbeddingAdapter {
            provider: embedder.clone(),
        });
        let exec_searcher = ExecutionSearcher::new(event_store.clone(), exec_index, exec_embedder);
        info!("Execution searcher initialized");

        let skill_embedder = Arc::new(SkillEmbeddingAdapter {
            provider: embedder.clone(),
        });
        let skill_router =
            SemanticSkillRouter::new(skill_registry.clone(), skill_index, skill_embedder);

        let indexed = skill_router.reindex_all().await.unwrap_or(0);
        info!(
            "Semantic skill router initialized with {} indexed skills",
            indexed
        );

        (Some(Arc::new(exec_searcher)), Some(Arc::new(skill_router)))
    } else {
        info!("Vector search not available, using keyword-only routing");
        (None, None)
    };

    let llm_router = resolve_llm_provider(&config.llm)?;
    let llm_provider: Arc<dyn LlmProvider> = llm_router.clone();
    info!("LLM provider initialized: {}", llm_provider.name());

    let mut tool_registry = ToolRegistry::new();
    // Convert security config to ExecConfig
    let exec_timeout_secs = config.security.exec.max_timeout_secs;
    let exec_config = {
        let sec = &config.security.exec;
        let mode = match sec.mode.as_str() {
            "strict" => ExecMode::Strict,
            _ => ExecMode::Permissive,
        };
        ExecConfig {
            mode,
            max_timeout_secs: sec.max_timeout_secs,
            extra_blocked_commands: sec.extra_blocked_commands.clone(),
            allowed_commands: sec.allowed_commands.clone(),
            blocked_paths: sec.blocked_paths.clone(),
        }
    };
    let builtins_config = BuiltinsConfig {
        exec: exec_config,
        ..BuiltinsConfig::default()
    };
    register_builtins_with_config(&mut tool_registry, &builtins_config);
    let tool_count = tool_registry.len();
    let tool_registry = Arc::new(tool_registry);
    info!("Tool registry initialized with {} tools", tool_count);

    let session_store: Arc<dyn SessionStore> = match RedisStore::new(&config.redis.url) {
        Ok(store) => {
            info!("Redis session store initialized");
            Arc::new(store)
        }
        Err(e) => {
            warn!("Redis unavailable, using in-memory session store: {}", e);
            Arc::new(cratos_core::MemoryStore::new())
        }
    };

    // ── Graph RAG Memory ──────────────────────────────────────────────
    let graph_memory: Option<Arc<GraphMemory>> = {
        let memory_db_path = data_dir.join("memory.db");
        match GraphMemory::from_path(&memory_db_path).await {
            Ok(gm) => {
                let gm = if let Some(ref embedder) = embedding_provider {
                    let dimensions = embedder.dimensions();
                    let memory_index_path = vectors_dir.join("memory");
                    match VectorIndex::open(&memory_index_path, IndexConfig::new(dimensions)) {
                        Ok(idx) => {
                            let bridge = Arc::new(VectorBridge::new(
                                embedder.clone(),
                                Arc::new(idx),
                            ));
                            info!("Graph RAG memory initialized with embedding search");
                            gm.with_vector_bridge(bridge)
                        }
                        Err(e) => {
                            warn!("Failed to open memory vector index: {e}, using graph-only");
                            gm
                        }
                    }
                } else {
                    info!("Graph RAG memory initialized (graph-only, no embeddings)");
                    gm
                };
                Some(Arc::new(gm))
            }
            Err(e) => {
                warn!("Failed to initialize Graph RAG memory: {e}");
                None
            }
        }
    };

    let shutdown_controller = ShutdownController::new();
    info!("Shutdown controller initialized (timeout: 30s)");

    let allow_high_risk = config.approval.default_mode == "never";
    let exec_timeout = std::time::Duration::from_secs(exec_timeout_secs);
    let runner_config = RunnerConfig::new(exec_timeout).with_high_risk(allow_high_risk);
    let orchestrator_config = OrchestratorConfig::new()
        .with_max_iterations(10)
        .with_logging(true)
        .with_planner_config({
            // Resolve actual provider name (not "router")
            let (prov_name, model_name) = if llm_provider.name() == "router" {
                // LlmRouter delegates to default provider
                let model = llm_provider.default_model();
                let name = if config.llm.default_provider.is_empty() || config.llm.default_provider == "auto" {
                    "auto-selected"
                } else {
                    config.llm.default_provider.as_str()
                };
                (name.to_string(), model.to_string())
            } else {
                (llm_provider.name().to_string(), llm_provider.default_model().to_string())
            };
            PlannerConfig::default()
                .with_machine_info()
                .with_provider_info(&prov_name, &model_name)
        })
        .with_runner_config(runner_config);

    let approval_manager = Arc::new(ApprovalManager::new());
    info!(
        "Approval manager initialized (mode: {})",
        config.approval.default_mode
    );

    let olympus_hooks = OlympusHooks::new(OlympusConfig::default());
    info!("Olympus OS hooks initialized");

    let event_bus = Arc::new(EventBus::new(256));
    info!("EventBus initialized (capacity: 256)");

    let mut orchestrator =
        Orchestrator::new(llm_provider.clone(), tool_registry, orchestrator_config)
            .with_event_store(event_store.clone())
            .with_event_bus(event_bus.clone())
            .with_memory(session_store)
            .with_approval_manager(approval_manager)
            .with_olympus_hooks(olympus_hooks)
            .with_persona_mapping(cratos_core::PersonaMapping::default_mapping())
            .with_agent_routing(cratos_core::AgentConfig::defaults());
    if let Some(gm) = graph_memory {
        orchestrator = orchestrator.with_graph_memory(gm);
    }

    // Phase 4: Auto-detect fallback provider
    {
        let primary = config.llm.default_provider.clone();
        let fallback_candidates = ["groq", "novita", "deepseek", "openrouter", "ollama"];
        if let Some(fb) = fallback_candidates.iter()
            .filter(|n| **n != primary.as_str())
            .find_map(|n| llm_router.get(n))
        {
            info!(fallback = %fb.name(), "Fallback LLM provider configured");
            orchestrator = orchestrator.with_fallback_provider(fb);
        }
    }

    // Phase 5: Connect semantic skill router
    if let Some(ref sr) = _semantic_skill_router {
        struct SkillRouterAdapter(Arc<SemanticSkillRouter<SkillEmbeddingAdapter>>);

        #[async_trait::async_trait]
        impl cratos_core::SkillRouting for SkillRouterAdapter {
            async fn route_best(&self, input: &str) -> Option<(String, String, f32)> {
                self.0.route_best(input).await
                    .map(|m| (m.skill.name, m.skill.description, m.score))
            }
        }

        orchestrator = orchestrator.with_skill_router(
            Arc::new(SkillRouterAdapter(sr.clone()))
        );
        info!("Skill router connected to orchestrator");
    }

    let orchestrator = Arc::new(orchestrator);
    info!("Orchestrator initialized");

    let mut channel_handles = Vec::new();

    if config.channels.telegram.enabled {
        match TelegramConfig::from_env() {
            Ok(telegram_config) => {
                let telegram_adapter = Arc::new(TelegramAdapter::new(telegram_config));
                let telegram_orchestrator = orchestrator.clone();
                let telegram_shutdown = shutdown_controller.token();

                let handle = tokio::spawn(async move {
                    tokio::select! {
                        result = telegram_adapter.run(telegram_orchestrator) => {
                            if let Err(e) = result {
                                error!("Telegram adapter error: {}", e);
                            }
                        }
                        _ = telegram_shutdown.cancelled() => {
                            info!("Telegram adapter shutting down...");
                        }
                    }
                });

                channel_handles.push(handle);
                info!("Telegram adapter started");
            }
            Err(e) => {
                warn!("Telegram adapter not started: {}", e);
            }
        }
    }

    // Phase 7: Slack adapter startup
    // Note: slack-morphism's Socket Mode listener produces !Send futures,
    // so we run it on a dedicated thread with its own Tokio LocalSet.
    if config.channels.slack.enabled {
        match cratos_channels::SlackConfig::from_env() {
            Ok(slack_config) => {
                let slack_adapter = Arc::new(cratos_channels::SlackAdapter::new(slack_config));
                let slack_orch = orchestrator.clone();
                let slack_shutdown = shutdown_controller.token();
                let handle = tokio::task::spawn_blocking(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("Failed to build Slack adapter runtime");
                    let local = tokio::task::LocalSet::new();
                    local.block_on(&rt, async move {
                        tokio::select! {
                            result = slack_adapter.run(slack_orch) => {
                                if let Err(e) = result { error!("Slack adapter error: {}", e); }
                            }
                            _ = slack_shutdown.cancelled() => {
                                info!("Slack adapter shutting down...");
                            }
                        }
                    });
                });
                channel_handles.push(handle);
                info!("Slack adapter started (dedicated thread)");
            }
            Err(e) => warn!("Slack adapter not started: {}", e),
        }
    }

    // Phase 6: ProactiveScheduler with real executor
    if config.scheduler.enabled {
        let scheduler_db_path = data_dir.join("scheduler.db");
        match SchedulerStore::from_path(&scheduler_db_path).await {
            Ok(scheduler_store) => {
                let scheduler_config = SchedulerConfig::default()
                    .with_check_interval(config.scheduler.check_interval_secs)
                    .with_retry_delay(config.scheduler.retry_delay_secs)
                    .with_max_concurrent(config.scheduler.max_concurrent);

                // Build real executor using orchestrator
                let sched_orch = orchestrator.clone();
                let task_executor: cratos_core::scheduler::TaskExecutor =
                    Arc::new(move |action: cratos_core::scheduler::TaskAction| {
                        let orch = sched_orch.clone();
                        Box::pin(async move {
                            use cratos_core::scheduler::TaskAction;
                            match action {
                                TaskAction::NaturalLanguage { prompt, channel } => {
                                    let input = cratos_core::OrchestratorInput::new(
                                        "scheduler",
                                        channel.as_deref().unwrap_or("system"),
                                        "system",
                                        &prompt,
                                    );
                                    orch.process(input)
                                        .await
                                        .map(|r| r.response)
                                        .map_err(|e| cratos_core::scheduler::SchedulerError::Execution(
                                            e.to_string(),
                                        ))
                                }
                                TaskAction::ToolCall { tool, args } => {
                                    orch.runner()
                                        .execute(&tool, args)
                                        .await
                                        .map(|r| {
                                            serde_json::to_string(&r.result.output)
                                                .unwrap_or_default()
                                        })
                                        .map_err(|e| cratos_core::scheduler::SchedulerError::Execution(
                                            e.to_string(),
                                        ))
                                }
                                _ => Ok("Action type not yet supported".to_string()),
                            }
                        })
                    });

                let scheduler_engine =
                    SchedulerEngine::new(Arc::new(scheduler_store), scheduler_config)
                        .with_executor(task_executor);
                let scheduler_shutdown = shutdown_controller.token();

                tokio::spawn(async move {
                    if let Err(e) = scheduler_engine.run(scheduler_shutdown).await {
                        error!("Scheduler error: {}", e);
                    }
                });

                info!(
                    "ProactiveScheduler started (check interval: {}s, max concurrent: {})",
                    config.scheduler.check_interval_secs, config.scheduler.max_concurrent
                );
            }
            Err(e) => {
                warn!("Failed to initialize scheduler store: {}", e);
            }
        }
    } else {
        info!("ProactiveScheduler disabled by configuration");
    }

    // Phase 3: Auto skill generation background task
    {
        let skill_event_store = event_store.clone();
        let skill_store_bg = skill_store.clone();
        let skill_registry_bg = skill_registry.clone();
        let skill_shutdown = shutdown_controller.token();

        tokio::spawn(async move {
            let analyzer = cratos_skills::PatternAnalyzer::new();
            let generator = cratos_skills::SkillGenerator::new();
            let interval = tokio::time::Duration::from_secs(3600);

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {
                        match analyzer.detect_patterns(&skill_event_store).await {
                            Ok(patterns) if !patterns.is_empty() => {
                                info!(count = patterns.len(), "Detected usage patterns");
                                for pattern in &patterns {
                                    match generator.generate_from_pattern(pattern) {
                                        Ok(skill) => {
                                            let name = skill.name.clone();
                                            if let Err(e) = skill_store_bg.save_skill(&skill).await {
                                                warn!(error = %e, "Failed to save auto-generated skill");
                                            } else {
                                                info!(skill = %name, "Auto-generated skill saved");
                                                let _ = skill_registry_bg.register(skill).await;
                                            }
                                        }
                                        Err(e) => debug!(error = %e, "Pattern skipped"),
                                    }
                                }
                            }
                            Ok(_) => debug!("No new patterns detected"),
                            Err(e) => warn!(error = %e, "Pattern analysis failed"),
                        }
                    }
                    _ = skill_shutdown.cancelled() => {
                        info!("Skill generation background task shutting down");
                        break;
                    }
                }
            }
        });
        info!("Auto skill generation background task started (1h interval)");
    }

    let cleanup_event_store = event_store.clone();
    let retention_days = config.replay.retention_days;
    let cleanup_shutdown = shutdown_controller.token();
    tokio::spawn(async move {
        let cleanup_interval = tokio::time::Duration::from_secs(3600);
        loop {
            tokio::select! {
                _ = tokio::time::sleep(cleanup_interval) => {
                    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
                    match cleanup_event_store.delete_old_executions(cutoff).await {
                        Ok(deleted) => {
                            if deleted > 0 {
                                info!("Cleanup: deleted {} old executions (retention: {} days)", deleted, retention_days);
                            }
                        }
                        Err(e) => {
                            warn!("Cleanup failed: {}", e);
                        }
                    }
                }
                _ = cleanup_shutdown.cancelled() => {
                    info!("Cleanup task shutting down");
                    break;
                }
            }
        }
    });
    info!("Cleanup task started (retention: {} days)", retention_days);

    let redis_url_for_health = config.redis.url.clone();

    // ================================================================
    // Authentication
    // ================================================================
    let auth_enabled = config.server.auth.enabled;
    let auth_store = Arc::new(AuthStore::new(auth_enabled));

    if auth_enabled && config.server.auth.auto_generate_key && auth_store.active_key_count() == 0 {
        // Auto-generate admin API key on first run
        match auth_store.generate_api_key("admin", admin_scopes(), "auto-generated admin key") {
            Ok((key, _hash)) => {
                info!("==========================================================");
                info!("  AUTO-GENERATED ADMIN API KEY (save this, shown once!):");
                info!("  {}", key.expose());
                info!("==========================================================");
            }
            Err(e) => {
                warn!("Failed to auto-generate API key: {}", e);
            }
        }
    }

    if auth_enabled {
        info!("Authentication ENABLED - API key required for all endpoints");
    } else {
        info!("Authentication DISABLED - all endpoints open (development mode)");
    }

    // ================================================================
    // Rate Limiting
    // ================================================================
    let rate_limit_layer = RateLimitLayer::new(&config.server.rate_limit);
    if config.server.rate_limit.enabled {
        rate_limit_layer.state().spawn_cleanup();
        info!(
            "Rate limiting ENABLED ({}rpm/token, {}rpm global)",
            config.server.rate_limit.requests_per_minute,
            config.server.rate_limit.global_requests_per_minute
        );
    } else {
        info!("Rate limiting DISABLED");
    }

    // ================================================================
    // Node Registry (Phase 9)
    // ================================================================
    let node_registry = Arc::new(cratos_core::NodeRegistry::new());
    info!("Node registry initialized");

    // ================================================================
    // A2A Router (Phase 11)
    // ================================================================
    let a2a_router = Arc::new(cratos_core::A2aRouter::default());
    info!("A2A router initialized");

    // Build the main router with all endpoints
    let app = Router::new()
        // Health and metrics endpoints (no auth required for load balancers)
        .route("/health", get(health_check))
        .route("/health/detailed", get(detailed_health_check))
        .route("/metrics", get(metrics_endpoint))
        // Root endpoint (no auth)
        .route("/", get(|| async { "Cratos AI Assistant" }))
        // API routes (auth applied per-handler via RequireAuth extractor)
        .merge(crate::api::api_router())
        // WebSocket routes
        .merge(crate::websocket::websocket_router())
        // Layers (applied to all routes)
        .layer(Extension(redis_url_for_health))
        .layer(Extension(auth_store))
        .layer(Extension(event_bus))
        .layer(Extension(node_registry))
        .layer(Extension(a2a_router))
        .layer(rate_limit_layer);

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .context("Invalid server address")?;

    info!("HTTP server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("Failed to bind to address")?;

    let server_shutdown = shutdown_controller.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal_with_controller(server_shutdown))
        .await
        .context("HTTP server error")?;

    info!("Waiting for channel adapters to finish...");
    let adapter_timeout = tokio::time::Duration::from_secs(5);
    for handle in channel_handles {
        match tokio::time::timeout(adapter_timeout, handle).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => warn!("Channel adapter task error: {}", e),
            Err(_) => warn!("Channel adapter shutdown timeout, aborting"),
        }
    }

    info!("Cratos shutdown complete");
    Ok(())
}
