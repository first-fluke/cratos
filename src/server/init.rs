//! Server initialization and main run loop
//!
//! Contains the main `run()` function that starts all server components.

use super::adapters::{EmbeddingAdapter, SkillEmbeddingAdapter};
use super::config::AppConfig;
use super::loader::load_config;
use super::providers::resolve_llm_provider;
use super::validation::validate_production_config;
use crate::middleware::rate_limit::RateLimitLayer;
use anyhow::{Context, Result};
use axum::{routing::get, Extension, Router};
use tower_http::services::ServeDir;
use cratos_channels::{DiscordAdapter, DiscordConfig, TelegramAdapter, TelegramConfig};
use cratos_core::{
    admin_scopes, shutdown_signal_with_controller, ApprovalManager, AuthStore, EventBus,
    OlympusConfig, OlympusHooks, Orchestrator, OrchestratorConfig, PlannerConfig, RedisStore,
    SchedulerConfig, SchedulerEngine, SchedulerStore, SessionStore, ShutdownController,
};
use cratos_llm::{EmbeddingProvider, LlmProvider, SharedEmbeddingProvider, TractEmbeddingProvider};
use cratos_memory::{GraphMemory, VectorBridge};
use cratos_replay::EventStore;
use cratos_search::{IndexConfig, VectorIndex};
use cratos_skills::{SemanticSkillRouter, SkillRegistry, SkillStore};
use cratos_tools::{
    register_builtins_with_config, BuiltinsConfig, ExecConfig, ExecMode, RunnerConfig, ToolRegistry,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

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
        .clone()
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

    // Initialize persona-skill store (same DB as skill_store)
    let persona_skill_store = Arc::new(
        cratos_skills::PersonaSkillStore::from_path(&skill_db_path)
            .await
            .context("Failed to initialize persona skill store")?,
    );
    info!("Persona skill store initialized");

    // Initialize default skills from persona TOML files
    init_default_skills(&persona_skill_store, &skill_store).await;

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
    let (_execution_searcher, _semantic_skill_router) =
        init_vector_search(&embedding_provider, &vectors_dir, &event_store, &skill_registry)
            .await?;

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
            allow_network_commands: false,
            ..ExecConfig::default()
        }
    };
    let builtins_config = BuiltinsConfig {
        exec: exec_config,
        ..BuiltinsConfig::default()
    };
    register_builtins_with_config(&mut tool_registry, &builtins_config);

    // Register application-level tools (bridge multiple crates)
    tool_registry.register(Arc::new(crate::tools::StatusTool::new(skill_store.clone())));

    // MCP tool auto-registration from .mcp.json
    let mcp_json_path = std::path::Path::new(".mcp.json");
    if mcp_json_path.exists() {
        match cratos_tools::mcp::register_mcp_tools(&mut tool_registry, mcp_json_path).await {
            Ok(_mcp_client) => {
                info!("MCP tools registered from .mcp.json");
            }
            Err(e) => warn!("Failed to register MCP tools: {}", e),
        }
    }

    // ── Graph RAG Memory ──────────────────────────────────────────────
    let graph_memory =
        init_graph_memory(&data_dir, &vectors_dir, &embedding_provider, &mut tool_registry).await;

    // ── Canvas State (live document editing) ──────────────────────────
    let canvas_state: Option<Arc<cratos_canvas::CanvasState>> = if config.canvas.enabled {
        let session_manager = Arc::new(cratos_canvas::CanvasSessionManager::new());
        let state = cratos_canvas::CanvasState::new(session_manager);
        info!(max_sessions = config.canvas.max_sessions, "Canvas state initialized");
        Some(Arc::new(state))
    } else {
        debug!("Canvas disabled by configuration");
        None
    };

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
                let name = if config.llm.default_provider.is_empty()
                    || config.llm.default_provider == "auto"
                {
                    "auto-selected"
                } else {
                    config.llm.default_provider.as_str()
                };
                (name.to_string(), model.to_string())
            } else {
                (
                    llm_provider.name().to_string(),
                    llm_provider.default_model().to_string(),
                )
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

    let mut orchestrator = Orchestrator::new(
        llm_provider.clone(),
        tool_registry.clone(),
        orchestrator_config,
    )
    .with_event_store(event_store.clone())
    .with_event_bus(event_bus.clone())
    .with_memory(session_store)
    .with_approval_manager(approval_manager)
    .with_olympus_hooks(olympus_hooks)
    .with_persona_mapping(cratos_core::PersonaMapping::default_mapping());

    if let Some(gm) = graph_memory {
        orchestrator = orchestrator.with_graph_memory(gm);
    }

    // Phase 4: Auto-detect fallback provider
    {
        let primary = config.llm.default_provider.clone();
        let fallback_candidates = [
            "groq",
            "openai",
            "novita",
            "deepseek",
            "anthropic",
            "openrouter",
            "ollama",
        ];
        if let Some(fb) = fallback_candidates
            .iter()
            .filter(|n| **n != primary.as_str())
            .find_map(|n| llm_router.get(n))
        {
            info!(fallback = %fb.name(), "Fallback LLM provider configured");
            orchestrator = orchestrator.with_fallback_provider(fb);
        }
    }

    // Phase 5: Connect semantic skill router (Phase 8: returns SkillMatch with skill_id)
    if let Some(ref sr) = _semantic_skill_router {
        struct SkillRouterAdapter(Arc<SemanticSkillRouter<SkillEmbeddingAdapter>>);

        #[async_trait::async_trait]
        impl cratos_core::SkillRouting for SkillRouterAdapter {
            async fn route_best(&self, input: &str) -> Option<cratos_core::SkillMatch> {
                self.0
                    .route_best(input)
                    .await
                    .map(|m| cratos_core::SkillMatch {
                        skill_id: m.skill.id,
                        skill_name: m.skill.name,
                        description: m.skill.description,
                        score: m.score,
                    })
            }
        }

        orchestrator = orchestrator.with_skill_router(Arc::new(SkillRouterAdapter(sr.clone())));
        info!("Skill router connected to orchestrator");
    }

    // Connect persona-skill store to orchestrator
    orchestrator = orchestrator.with_persona_skill_store(persona_skill_store.clone());
    info!("Persona skill store connected to orchestrator");

    let orchestrator = Arc::new(orchestrator);
    info!("Orchestrator initialized");

    // Dev Session Monitor (AI session detection) - created early for channel use
    let dev_monitor = Arc::new(cratos_core::DevSessionMonitor::new(
        std::time::Duration::from_secs(30),
    ));
    dev_monitor.clone().start();
    info!("Dev session monitor started (30s poll)");

    let mut channel_handles = Vec::new();

    // Start Telegram adapter
    if config.channels.telegram.enabled {
        if let Some(handle) =
            start_telegram_adapter(&orchestrator, &dev_monitor, &shutdown_controller)
        {
            channel_handles.push(handle);
        }
    }

    // Start Slack adapter
    if config.channels.slack.enabled {
        if let Some(handle) = start_slack_adapter(&orchestrator, &shutdown_controller) {
            channel_handles.push(handle);
        }
    }

    // Start WhatsApp adapter (Baileys bridge)
    if config.channels.whatsapp.enabled {
        if let Some(handle) = start_whatsapp_adapter(&shutdown_controller) {
            channel_handles.push(handle);
        }
    }

    // Start Discord adapter
    if config.channels.discord.enabled {
        if let Some(handle) = start_discord_adapter(&orchestrator, &shutdown_controller) {
            channel_handles.push(handle);
        }
    }

    // Start Matrix adapter
    if config.channels.matrix.enabled {
        if let Some(handle) = start_matrix_adapter(&orchestrator, &shutdown_controller) {
            channel_handles.push(handle);
        }
    }

    // Phase 6: ProactiveScheduler with real executor
    let scheduler_engine_ext =
        start_scheduler(&config, &data_dir, &orchestrator, &shutdown_controller).await;

    // Phase 3: Auto skill generation background task
    start_skill_generation_task(&event_store, &skill_store, &skill_registry, &shutdown_controller);

    // Cleanup task
    start_cleanup_task(&event_store, config.replay.retention_days, &shutdown_controller);

    let redis_url_for_health = config.redis.url.clone();

    // ================================================================
    // Authentication
    // ================================================================
    let auth_store = init_auth(&config);

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

    // ================================================================
    // Browser Relay (Extension ↔ Server bridge)
    // ================================================================
    let browser_relay: crate::websocket::gateway::SharedBrowserRelay =
        Arc::new(crate::websocket::gateway::BrowserRelay::new());
    info!("Browser relay initialized");

    // PairingManager (PIN-based device pairing) — try SQLite, fall back to in-memory
    let pairing_manager =
        match cratos_core::pairing::PairingManager::new_with_db(event_store.pool().clone()).await {
            Ok(mgr) => {
                info!("PairingManager initialized with SQLite persistence");
                Arc::new(mgr)
            }
            Err(e) => {
                warn!("PairingManager SQLite init failed ({}), using in-memory", e);
                Arc::new(cratos_core::pairing::PairingManager::new())
            }
        };

    // ChallengeStore for device challenge-response auth
    let challenge_store = Arc::new(cratos_core::device_auth::ChallengeStore::new());
    info!("ChallengeStore initialized (TTL: 60s)");

    // E2E Session State (shared between REST sessions API and WS chat)
    let session_state = crate::api::SessionState::new();
    let e2e_ciphers = session_state.e2e_ciphers();

    // Initialize WhatsApp Business adapter (for webhook handling)
    let whatsapp_business_adapter: Option<Arc<cratos_channels::WhatsAppBusinessAdapter>> =
        if config.channels.whatsapp_business.enabled {
            match cratos_channels::WhatsAppBusinessConfig::from_env() {
                Ok(cfg) => match cratos_channels::WhatsAppBusinessAdapter::new(cfg) {
                    Ok(adapter) => {
                        info!("WhatsApp Business adapter initialized");
                        Some(Arc::new(adapter))
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to create WhatsApp Business adapter");
                        None
                    }
                },
                Err(e) => {
                    warn!(error = %e, "WhatsApp Business not configured");
                    None
                }
            }
        } else {
            None
        };

    // Web UI static files (SPA fallback)
    let web_ui_dir = std::path::Path::new("apps/web/dist");
    let serve_web_ui = web_ui_dir.exists();
    if serve_web_ui {
        info!("Web UI enabled: serving from {}", web_ui_dir.display());
    }

    // Build the main router with all endpoints
    let app = Router::new()
        // Health endpoints (/health public for LB, /health/detailed and /metrics require auth)
        .merge(crate::api::health_routes())
        // Root endpoint (no auth) - only if Web UI is not enabled
        .route("/", get(|| async { "Cratos AI Assistant" }))
        // API routes (auth applied per-handler via RequireAuth extractor)
        .merge(crate::api::api_router_with_session_state(session_state))
        // WebSocket routes
        .merge(crate::websocket::websocket_router())
        // Layers (applied to all routes)
        .layer(Extension(redis_url_for_health))
        .layer(Extension(auth_store))
        .layer(Extension(event_bus))
        .layer(Extension(node_registry))
        .layer(Extension(a2a_router))
        .layer(Extension(browser_relay))
        .layer(Extension(orchestrator.clone()))
        .layer(Extension(tool_registry.clone()))
        .layer(Extension(dev_monitor.clone()))
        .layer(Extension(event_store.clone()))
        .layer(Extension(skill_store.clone()))
        .layer(Extension(e2e_ciphers))
        .layer(Extension(pairing_manager))
        .layer(Extension(challenge_store))
        .layer(rate_limit_layer);

    // Conditionally add scheduler engine Extension
    let app = if let Some(sched_engine) = scheduler_engine_ext {
        app.layer(Extension(sched_engine))
    } else {
        app
    };

    // Conditionally add Canvas state Extension
    let app = if let Some(canvas) = canvas_state {
        app.layer(Extension(canvas))
    } else {
        app
    };

    // Conditionally add WhatsApp Business adapter Extension
    let app = if let Some(wa_adapter) = whatsapp_business_adapter {
        app.layer(Extension(wa_adapter))
    } else {
        app
    };

    // Add Web UI static file serving (SPA fallback)
    let app = if serve_web_ui {
        // Serve static files, fallback to index.html for SPA routing
        let serve_dir = ServeDir::new(web_ui_dir)
            .append_index_html_on_directories(true)
            .fallback(tower_http::services::ServeFile::new(
                web_ui_dir.join("index.html"),
            ));
        app.fallback_service(serve_dir)
    } else {
        app
    };

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .context("Invalid server address")?;

    info!("HTTP server listening on http://{}", addr);

    // mDNS service discovery (advertise on LAN)
    let discovery = cratos_core::DiscoveryService::new(cratos_core::DiscoveryConfig::default());
    if let Err(e) = discovery.start(config.server.port) {
        warn!("mDNS discovery failed to start: {}", e);
    }

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("Failed to bind to address")?;

    let server_shutdown = shutdown_controller.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal_with_controller(server_shutdown))
        .await
        .context("HTTP server error")?;

    discovery.stop();

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

// ════════════════════════════════════════════════════════════════════
// Helper functions to reduce run() complexity
// ════════════════════════════════════════════════════════════════════

async fn init_default_skills(
    persona_skill_store: &Arc<cratos_skills::PersonaSkillStore>,
    skill_store: &Arc<SkillStore>,
) {
    let persona_loader = cratos_core::pantheon::PersonaLoader::new();
    let persona_mapping = cratos_core::PersonaMapping::from_loader(&persona_loader);
    let mut default_skills_registered = 0usize;

    for persona_name in persona_mapping.persona_names() {
        if let Some(preset) = persona_mapping.get_preset(persona_name) {
            for skill_name in &preset.skills.default {
                // Skip if already bound
                match persona_skill_store
                    .has_skill_by_name(persona_name, skill_name)
                    .await
                {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(e) => {
                        warn!(
                            persona = %persona_name,
                            skill = %skill_name,
                            error = %e,
                            "Failed to check skill binding"
                        );
                        continue;
                    }
                }

                // Find skill by name and create default binding
                if let Ok(Some(skill)) = skill_store.get_skill_by_name(skill_name).await {
                    if let Err(e) = persona_skill_store
                        .create_default_binding(persona_name, skill.id, skill_name)
                        .await
                    {
                        warn!(
                            persona = %persona_name,
                            skill = %skill_name,
                            error = %e,
                            "Failed to create default skill binding"
                        );
                    } else {
                        default_skills_registered += 1;
                    }
                }
            }
        }
    }

    if default_skills_registered > 0 {
        info!(
            "Registered {} default persona-skill bindings from TOML",
            default_skills_registered
        );
    }
}

type ExecutionSearcherOpt = Option<Arc<cratos_replay::ExecutionSearcher<EmbeddingAdapter>>>;
type SkillRouterOpt = Option<Arc<SemanticSkillRouter<SkillEmbeddingAdapter>>>;

async fn init_vector_search(
    embedding_provider: &Option<SharedEmbeddingProvider>,
    vectors_dir: &std::path::Path,
    event_store: &Arc<EventStore>,
    skill_registry: &Arc<SkillRegistry>,
) -> Result<(ExecutionSearcherOpt, SkillRouterOpt)> {
    if let Some(ref embedder) = embedding_provider {
        std::fs::create_dir_all(vectors_dir).context("Failed to create vectors directory")?;

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
        let exec_searcher =
            cratos_replay::ExecutionSearcher::new(event_store.clone(), exec_index, exec_embedder);
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

        Ok((Some(Arc::new(exec_searcher)), Some(Arc::new(skill_router))))
    } else {
        info!("Vector search not available, using keyword-only routing");
        Ok((None, None))
    }
}

async fn init_graph_memory(
    data_dir: &std::path::Path,
    vectors_dir: &std::path::Path,
    embedding_provider: &Option<SharedEmbeddingProvider>,
    tool_registry: &mut ToolRegistry,
) -> Option<Arc<GraphMemory>> {
    let memory_db_path = data_dir.join("memory.db");
    match GraphMemory::from_path(&memory_db_path).await {
        Ok(gm) => {
            let gm = if let Some(ref embedder) = embedding_provider {
                let dimensions = embedder.dimensions();
                // Turn embedding index
                let memory_index_path = vectors_dir.join("memory");
                let gm = match VectorIndex::open(&memory_index_path, IndexConfig::new(dimensions)) {
                    Ok(idx) => {
                        let bridge = Arc::new(VectorBridge::new(embedder.clone(), Arc::new(idx)));
                        info!("Graph RAG memory initialized with embedding search");
                        gm.with_vector_bridge(bridge)
                    }
                    Err(e) => {
                        warn!("Failed to open memory vector index: {e}, using graph-only");
                        gm
                    }
                };
                // Explicit memory embedding index (separate HNSW)
                let explicit_index_path = vectors_dir.join("explicit");
                match VectorIndex::open(&explicit_index_path, IndexConfig::new(dimensions)) {
                    Ok(idx) => {
                        let bridge = Arc::new(VectorBridge::new(embedder.clone(), Arc::new(idx)));
                        info!("Explicit memory vector index initialized");
                        gm.with_explicit_vector_bridge(bridge)
                    }
                    Err(e) => {
                        warn!("Failed to open explicit memory vector index: {e}");
                        gm
                    }
                }
            } else {
                info!("Graph RAG memory initialized (graph-only, no embeddings)");
                gm
            };
            let gm = Arc::new(gm);

            // Register memory tool (explicit save/recall)
            tool_registry.register(Arc::new(crate::tools::MemoryTool::new(Arc::clone(&gm))));
            // Backfill: embed any explicit memories missing from vector index
            if let Err(e) = gm.reindex_explicit_memories().await {
                warn!("Failed to reindex explicit memories: {e}");
            }

            Some(gm)
        }
        Err(e) => {
            warn!("Failed to initialize Graph RAG memory: {e}");
            None
        }
    }
}

fn start_telegram_adapter(
    orchestrator: &Arc<Orchestrator>,
    dev_monitor: &Arc<cratos_core::DevSessionMonitor>,
    shutdown_controller: &ShutdownController,
) -> Option<tokio::task::JoinHandle<()>> {
    match TelegramConfig::from_env() {
        Ok(telegram_config) => {
            let telegram_adapter = Arc::new(TelegramAdapter::new(telegram_config));
            let telegram_orchestrator = orchestrator.clone();
            let telegram_dev_monitor = Some(dev_monitor.clone());
            let telegram_shutdown = shutdown_controller.token();

            let handle = tokio::spawn(async move {
                tokio::select! {
                    result = telegram_adapter.run(telegram_orchestrator, telegram_dev_monitor) => {
                        if let Err(e) = result {
                            error!("Telegram adapter error: {}", e);
                        }
                    }
                    _ = telegram_shutdown.cancelled() => {
                        info!("Telegram adapter shutting down...");
                    }
                }
            });

            info!("Telegram adapter started");
            Some(handle)
        }
        Err(e) => {
            warn!("Telegram adapter not started: {}", e);
            None
        }
    }
}

fn start_slack_adapter(
    orchestrator: &Arc<Orchestrator>,
    shutdown_controller: &ShutdownController,
) -> Option<tokio::task::JoinHandle<()>> {
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
            info!("Slack adapter started (dedicated thread)");
            Some(handle)
        }
        Err(e) => {
            warn!("Slack adapter not started: {}", e);
            None
        }
    }
}

/// Start the Matrix adapter
///
/// Connects to a Matrix homeserver and handles room messages.
/// Returns None if configuration is missing or adapter creation fails.
fn start_matrix_adapter(
    orchestrator: &Arc<Orchestrator>,
    shutdown_controller: &ShutdownController,
) -> Option<tokio::task::JoinHandle<()>> {
    use cratos_channels::{MatrixAdapter, MatrixConfig};

    let config = match MatrixConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Matrix adapter not started: missing configuration");
            return None;
        }
    };

    let adapter = match MatrixAdapter::new(config) {
        Ok(a) => Arc::new(a),
        Err(e) => {
            warn!(error = %e, "Failed to create Matrix adapter");
            return None;
        }
    };

    let orch = orchestrator.clone();
    let shutdown = shutdown_controller.token();

    let handle = tokio::spawn(async move {
        if let Err(e) = adapter.run(orch, shutdown).await {
            error!(error = %e, "Matrix adapter error");
        }
    });

    info!("Matrix adapter started");
    Some(handle)
}

/// Start the Discord adapter
///
/// Creates a Discord bot using serenity and starts the event loop.
/// Returns None if configuration is missing or adapter creation fails.
fn start_discord_adapter(
    orchestrator: &Arc<Orchestrator>,
    shutdown_controller: &ShutdownController,
) -> Option<tokio::task::JoinHandle<()>> {
    let config = match DiscordConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Discord adapter not started: missing configuration");
            return None;
        }
    };

    let adapter = Arc::new(DiscordAdapter::new(config));
    let orch = orchestrator.clone();
    let shutdown = shutdown_controller.token();

    let handle = tokio::spawn(async move {
        tokio::select! {
            result = adapter.run(orch) => {
                if let Err(e) = result {
                    error!(error = %e, "Discord adapter error");
                }
            }
            _ = shutdown.cancelled() => {
                info!("Discord adapter shutting down");
            }
        }
    });

    info!("Discord adapter started");
    Some(handle)
}

/// Start the WhatsApp (Baileys) adapter
///
/// Connects to the Baileys bridge server and periodically checks connection status.
/// Returns None if configuration is missing or adapter creation fails.
fn start_whatsapp_adapter(
    shutdown_controller: &ShutdownController,
) -> Option<tokio::task::JoinHandle<()>> {
    use cratos_channels::{WhatsAppAdapter, WhatsAppConfig};

    let config = match WhatsAppConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "WhatsApp adapter not started: missing configuration");
            return None;
        }
    };

    let adapter = match WhatsAppAdapter::new(config) {
        Ok(a) => Arc::new(a),
        Err(e) => {
            warn!(error = %e, "Failed to create WhatsApp adapter");
            return None;
        }
    };

    let shutdown = shutdown_controller.token();

    let handle = tokio::spawn(async move {
        // Baileys bridge polling mode - check connection status periodically
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match adapter.status().await {
                        Ok(status) => {
                            debug!(status = ?status, "WhatsApp connection status");
                        }
                        Err(e) => {
                            warn!(error = %e, "WhatsApp status check failed");
                        }
                    }
                }
                _ = shutdown.cancelled() => {
                    info!("WhatsApp adapter shutting down");
                    if let Err(e) = adapter.disconnect().await {
                        warn!(error = %e, "Error disconnecting WhatsApp");
                    }
                    break;
                }
            }
        }
    });

    info!("WhatsApp (Baileys) adapter started");
    Some(handle)
}

async fn start_scheduler(
    config: &AppConfig,
    data_dir: &std::path::Path,
    orchestrator: &Arc<Orchestrator>,
    shutdown_controller: &ShutdownController,
) -> Option<Arc<SchedulerEngine>> {
    if !config.scheduler.enabled {
        info!("ProactiveScheduler disabled by configuration");
        return None;
    }

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
                                orch.process(input).await.map(|r| r.response).map_err(|e| {
                                    cratos_core::scheduler::SchedulerError::Execution(e.to_string())
                                })
                            }
                            TaskAction::ToolCall { tool, args } => orch
                                .runner()
                                .execute(&tool, args)
                                .await
                                .map(|r| serde_json::to_string(&r.result.output).unwrap_or_default())
                                .map_err(|e| {
                                    cratos_core::scheduler::SchedulerError::Execution(e.to_string())
                                }),
                            TaskAction::Shell { command, cwd } => {
                                // Route shell commands through exec tool to apply
                                // security filters (blocked_commands, blocked_paths, injection defense)
                                let mut args = serde_json::json!({ "command": command });
                                if let Some(dir) = cwd {
                                    args["cwd"] = serde_json::Value::String(dir);
                                }
                                orch.runner()
                                    .execute("exec", args)
                                    .await
                                    .map(|r| {
                                        serde_json::to_string(&r.result.output).unwrap_or_default()
                                    })
                                    .map_err(|e| {
                                        cratos_core::scheduler::SchedulerError::Execution(
                                            e.to_string(),
                                        )
                                    })
                            }
                            _ => Ok("Action type not yet supported".to_string()),
                        }
                    })
                });

            let scheduler_engine = Arc::new(
                SchedulerEngine::new(Arc::new(scheduler_store), scheduler_config)
                    .with_executor(task_executor),
            );
            let scheduler_shutdown = shutdown_controller.token();

            // Clone for the background run loop
            let engine_for_run = scheduler_engine.clone();
            tokio::spawn(async move {
                if let Err(e) = engine_for_run.run(scheduler_shutdown).await {
                    error!("Scheduler error: {}", e);
                }
            });

            info!(
                "ProactiveScheduler started (check interval: {}s, max concurrent: {})",
                config.scheduler.check_interval_secs, config.scheduler.max_concurrent
            );

            Some(scheduler_engine)
        }
        Err(e) => {
            warn!("Failed to initialize scheduler store: {}", e);
            None
        }
    }
}

fn start_skill_generation_task(
    event_store: &Arc<EventStore>,
    skill_store: &Arc<SkillStore>,
    skill_registry: &Arc<SkillRegistry>,
    shutdown_controller: &ShutdownController,
) {
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

fn start_cleanup_task(
    event_store: &Arc<EventStore>,
    retention_days: u32,
    shutdown_controller: &ShutdownController,
) {
    let cleanup_event_store = event_store.clone();
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
}

fn init_auth(config: &AppConfig) -> Arc<AuthStore> {
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
        warn!("SECURITY: Authentication disabled — all API endpoints open (development only). Enable [server.auth] enabled = true for production.");
    }

    auth_store
}
