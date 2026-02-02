//! Cratos - AI-Powered Personal Assistant
//!
//! CLI entry point for the Cratos server.

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use axum::{routing::get, Extension, Json, Router};
use config::{Config, Environment, File};
use cratos_channels::{TelegramAdapter, TelegramConfig};
use cratos_core::{
    metrics_global, Orchestrator, OrchestratorConfig, PlannerConfig, RedisStore, SessionStore,
};
use cratos_llm::{AnthropicConfig, AnthropicProvider, LlmProvider, OpenAiConfig, OpenAiProvider};
use cratos_replay::EventStore;
use cratos_tools::{register_builtins, RunnerConfig, ToolRegistry};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Application configuration
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub llm: LlmConfig,
    pub approval: ApprovalConfig,
    pub replay: ReplayConfig,
    pub channels: ChannelsConfig,
}

/// Server configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

/// Database configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

/// Redis configuration
#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

/// LLM configuration
#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    pub default_provider: String,
    pub openai: Option<OpenAiLlmConfig>,
    pub anthropic: Option<AnthropicLlmConfig>,
    pub routing: Option<RoutingConfig>,
}

/// OpenAI-specific config
#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiLlmConfig {
    pub default_model: String,
}

/// Anthropic-specific config
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicLlmConfig {
    pub default_model: String,
}

/// Routing configuration for model selection
#[derive(Debug, Clone, Deserialize)]
pub struct RoutingConfig {
    pub classification: Option<String>,
    pub planning: Option<String>,
    pub code_generation: Option<String>,
    pub summarization: Option<String>,
}

/// Approval configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ApprovalConfig {
    pub default_mode: String,
}

/// Replay configuration
#[derive(Debug, Clone, Deserialize)]
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
    db_pool: axum::extract::Extension<sqlx::PgPool>,
    redis_url: axum::extract::Extension<String>,
) -> Json<DetailedHealthResponse> {
    // Check database
    let db_health = {
        let start = std::time::Instant::now();
        match sqlx::query("SELECT 1").execute(&*db_pool).await {
            Ok(_) => ComponentHealth::healthy(start.elapsed().as_millis() as u64),
            Err(e) => ComponentHealth::unhealthy(e.to_string()),
        }
    };

    // Check Redis
    let redis_health = {
        let start = std::time::Instant::now();
        match redis::Client::open(redis_url.as_str()) {
            Ok(client) => match client.get_multiplexed_async_connection().await {
                Ok(mut conn) => {
                    match redis::cmd("PING").query_async::<_, String>(&mut conn).await {
                        Ok(_) => ComponentHealth::healthy(start.elapsed().as_millis() as u64),
                        Err(e) => ComponentHealth::unhealthy(e.to_string()),
                    }
                }
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

/// Load configuration from files and environment
fn load_config() -> Result<AppConfig> {
    let config = Config::builder()
        // Load default configuration
        .add_source(File::with_name("config/default").required(true))
        // Load local overrides (optional)
        .add_source(File::with_name("config/local").required(false))
        // Load environment-specific config (optional)
        .add_source(
            File::with_name(&format!(
                "config/{}",
                std::env::var("CRATOS_ENV").unwrap_or_else(|_| "development".to_string())
            ))
            .required(false),
        )
        // Override with environment variables (CRATOS_ prefix)
        .add_source(
            Environment::with_prefix("CRATOS")
                .separator("__")
                .try_parsing(true),
        )
        .build()
        .context("Failed to build configuration")?;

    config
        .try_deserialize()
        .context("Failed to deserialize configuration")
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file (if present)
    let _ = dotenvy::dotenv();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cratos=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!(
        "Starting Cratos AI Assistant v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Load configuration
    let config = load_config().context("Failed to load configuration")?;
    info!("Configuration loaded");

    // Initialize database connection pool
    let db_pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .connect(&config.database.url)
        .await
        .context("Failed to connect to database")?;
    info!("Database connection established");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .context("Failed to run database migrations")?;
    info!("Database migrations completed");

    // Initialize event store
    let event_store = Arc::new(EventStore::new(db_pool.clone()));
    info!("Event store initialized");

    // Initialize LLM provider based on configuration
    let llm_provider: Arc<dyn LlmProvider> = match config.llm.default_provider.as_str() {
        "openai" => {
            let openai_config =
                OpenAiConfig::from_env().context("Failed to load OpenAI configuration")?;
            Arc::new(OpenAiProvider::new(openai_config))
        }
        _ => {
            // Default to Anthropic
            let anthropic_config =
                AnthropicConfig::from_env().context("Failed to load Anthropic configuration")?;
            Arc::new(AnthropicProvider::new(anthropic_config)?)
        }
    };
    info!("LLM provider initialized: {}", llm_provider.name());

    // Initialize tool registry
    let mut tool_registry = ToolRegistry::new();
    register_builtins(&mut tool_registry);
    let tool_count = tool_registry.len();
    let tool_registry = Arc::new(tool_registry);
    info!("Tool registry initialized with {} tools", tool_count);

    // Initialize session store (Redis)
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

    // Initialize orchestrator
    let orchestrator_config = OrchestratorConfig::new()
        .with_max_iterations(10)
        .with_logging(true)
        .with_planner_config(PlannerConfig::default())
        .with_runner_config(RunnerConfig::default());

    let orchestrator = Arc::new(
        Orchestrator::new(llm_provider, tool_registry, orchestrator_config)
            .with_event_store(event_store)
            .with_memory(session_store),
    );
    info!("Orchestrator initialized");

    // Start channel adapters
    let mut channel_handles = Vec::new();

    // Telegram adapter
    if config.channels.telegram.enabled {
        match TelegramConfig::from_env() {
            Ok(telegram_config) => {
                let telegram_adapter = Arc::new(TelegramAdapter::new(telegram_config));
                let telegram_orchestrator = orchestrator.clone();

                let handle = tokio::spawn(async move {
                    if let Err(e) = telegram_adapter.run(telegram_orchestrator).await {
                        error!("Telegram adapter error: {}", e);
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

    // Slack adapter (if enabled)
    if config.channels.slack.enabled {
        info!("Slack adapter enabled but not yet started (requires additional setup)");
        // Note: Slack adapter requires additional tokens and socket mode setup
        // Uncomment when ready:
        // match SlackConfig::from_env() {
        //     Ok(slack_config) => {
        //         let slack_adapter = Arc::new(SlackAdapter::new(slack_config));
        //         let slack_orchestrator = orchestrator.clone();
        //
        //         let handle = tokio::spawn(async move {
        //             if let Err(e) = slack_adapter.run(slack_orchestrator).await {
        //                 error!("Slack adapter error: {}", e);
        //             }
        //         });
        //
        //         channel_handles.push(handle);
        //         info!("Slack adapter started");
        //     }
        //     Err(e) => {
        //         warn!("Slack adapter not started: {}", e);
        //     }
        // }
    }

    // Build HTTP server for health checks and webhooks
    let redis_url_for_health = config.redis.url.clone();
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/health/detailed", get(detailed_health_check))
        .route("/api/v1/health", get(health_check))
        .route("/metrics", get(metrics_endpoint))
        .route("/", get(|| async { "Cratos AI Assistant" }))
        .layer(Extension(db_pool.clone()))
        .layer(Extension(redis_url_for_health));

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .context("Invalid server address")?;

    info!("HTTP server listening on http://{}", addr);

    // Start HTTP server
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("Failed to bind to address")?;

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("HTTP server error")?;

    // Wait for channel adapters to finish
    for handle in channel_handles {
        let _ = handle.await;
    }

    info!("Cratos shutdown complete");
    Ok(())
}

/// Shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down");
        }
        _ = terminate => {
            info!("Received terminate signal, shutting down");
        }
    }
}
