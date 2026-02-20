//! Server initialization and main run loop
//!
//! Contains the main `run()` function that starts all server components.

use super::a2ui_steering::start_a2ui_steering_loop;
use super::adapters::SkillRouterAdapter;
use super::background_tasks::{start_cleanup_task, start_scheduler, start_skill_generation_task};
use super::channel_starters::{
    start_discord_adapter, start_matrix_adapter, start_slack_adapter, start_telegram_adapter,
    start_whatsapp_adapter,
};
use super::init_helpers::{
    init_auth, init_embedding_provider, init_graph_memory, init_vector_search,
};
use super::init_stores::init_stores;
use super::loader::load_config;
use super::providers::resolve_llm_provider;
use super::skill_init::init_default_skills;
use super::validation::validate_production_config;
use crate::middleware::rate_limit::RateLimitLayer;
use anyhow::{Context, Result};
use axum::{routing::get, Extension, Router};
use cratos_core::{
    shutdown_signal_with_controller, ApprovalManager, EventBus, OlympusConfig, OlympusHooks,
    Orchestrator, OrchestratorConfig, PlannerConfig, RedisStore, SessionStore, ShutdownController,
};
use cratos_llm::LlmProvider;
use cratos_tools::{
    register_builtins_with_config, BuiltinsConfig, ExecConfig, ExecMode, RunnerConfig, ToolRegistry,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{debug, info, warn};

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

    // Initialize all data stores
    let stores = init_stores(&data_dir).await?;
    let event_store = stores.event_store;
    let chronicle_store = stores.chronicle_store;
    let skill_store = stores.skill_store;
    let skill_registry = stores.skill_registry;
    let persona_skill_store = stores.persona_skill_store;

    // Initialize default skills from persona TOML files
    init_default_skills(&persona_skill_store, &skill_store).await;

    let embedding_provider = init_embedding_provider(&config);

    let vectors_dir = data_dir.join("vectors");
    let (_execution_searcher, _semantic_skill_router) = init_vector_search(
        &embedding_provider,
        &vectors_dir,
        &event_store,
        &skill_registry,
    )
    .await?;

    let llm_router = resolve_llm_provider(&config.llm)?;
    let llm_provider: Arc<dyn LlmProvider> = llm_router.clone();
    info!("LLM provider initialized: {}", llm_provider.name());

    // ── Gemini Quota Poller ──────────────────────────────────────────
    let _gemini_quota_tx = cratos_llm::start_gemini_quota_poller().await;
    if _gemini_quota_tx.is_some() {
        info!("Gemini quota poller started");
    }

    // ── Canvas State (live document editing) ──────────────────────────
    let (a2ui_tx, a2ui_rx) = mpsc::channel(100);

    let canvas_state: Option<Arc<cratos_canvas::CanvasState>> = if config.canvas.enabled {
        let session_manager = Arc::new(cratos_canvas::CanvasSessionManager::new());
        let state = cratos_canvas::CanvasState::new(session_manager).with_a2ui_tx(a2ui_tx);
        info!(
            max_sessions = config.canvas.max_sessions,
            "Canvas state initialized with A2UI Steering channel"
        );
        Some(Arc::new(state))
    } else {
        debug!("Canvas disabled by configuration");
        None
    };

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

    // Initialize A2UI Session Manager if Canvas is enabled
    let a2ui_manager = canvas_state
        .as_ref()
        .map(|state| Arc::new(cratos_canvas::a2ui::A2uiSessionManager::new(state.clone())));

    // ================================================================
    // A2A Router (Phase 11) - Logically moved up for tool injection
    // ================================================================
    let a2a_router = Arc::new(cratos_core::A2aRouter::default());

    let builtins_config = BuiltinsConfig {
        exec: exec_config,
        a2ui_manager,
        session_sender: Some(a2a_router.clone()), // Injected A2A router
        ..BuiltinsConfig::default()
    };
    register_builtins_with_config(&mut tool_registry, &builtins_config);

    // Register application-level tools (bridge multiple crates)
    tool_registry.register(Arc::new(crate::tools::StatusTool::new(skill_store.clone())));
    tool_registry.register(Arc::new(crate::tools::PersonaTool::new()));

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
    let graph_memory = init_graph_memory(
        &data_dir,
        &vectors_dir,
        &embedding_provider,
        &mut tool_registry,
    )
    .await;

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

    // Clone graph_memory for Extension before moving it to orchestrator
    let graph_memory_ext = graph_memory.clone();
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
        orchestrator = orchestrator.with_skill_router(Arc::new(SkillRouterAdapter(sr.clone())));
        info!("Skill router connected to orchestrator");
    }

    // Connect persona-skill store to orchestrator
    orchestrator = orchestrator.with_persona_skill_store(persona_skill_store.clone());
    info!("Persona skill store connected to orchestrator");

    // Connect chronicle store to orchestrator
    orchestrator = orchestrator.with_chronicle_store(chronicle_store.clone());
    info!("Chronicle store connected to orchestrator");

    let orchestrator = Arc::new(orchestrator);
    info!("Orchestrator initialized");

    // Start A2UI Steering Loop
    start_a2ui_steering_loop(orchestrator.clone(), a2ui_rx);

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
    let scheduler_engine_ext = start_scheduler(
        &config,
        &data_dir,
        &orchestrator,
        &event_bus,
        &skill_store,
        &shutdown_controller,
    )
    .await;

    // Phase 3: Auto skill generation background task
    start_skill_generation_task(
        &event_store,
        &skill_store,
        &skill_registry,
        &shutdown_controller,
    );

    // Cleanup task
    start_cleanup_task(
        &event_store,
        config.replay.retention_days,
        &shutdown_controller,
    );

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
    let node_registry = Arc::new(cratos_core::NodeRegistry::new(event_store.pool().clone()));
    info!("Node registry initialized");

    // ================================================================
    // A2A Router (Phase 11) - Moved up
    // ================================================================
    // a2a_router is already initialized above
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
    let config_state = crate::api::config::ConfigState::with_config(&config);
    let app = Router::new()
        // Health endpoints (/health public for LB, /health/detailed and /metrics require auth)
        .merge(crate::api::health_routes())
        // API documentation (Swagger UI at /docs)
        .merge(crate::api::docs_routes())
        // API routes (auth applied per-handler via RequireAuth extractor)
        .merge(crate::api::api_router_with_state(
            session_state,
            config_state,
        ))
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
        .layer(Extension(persona_skill_store.clone()))
        .layer(Extension(graph_memory_ext))
        .layer(Extension(e2e_ciphers))
        .layer(Extension(pairing_manager))
        .layer(Extension(challenge_store))
        .layer(rate_limit_layer)
        .layer(CorsLayer::permissive());

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

    // Add Web UI static file serving (SPA fallback) or simple text response
    let app = if serve_web_ui {
        // Serve static files, fallback to index.html for SPA routing
        let serve_dir = ServeDir::new(web_ui_dir)
            .append_index_html_on_directories(true)
            .fallback(tower_http::services::ServeFile::new(
                web_ui_dir.join("index.html"),
            ));
        app.fallback_service(serve_dir)
    } else {
        // No Web UI - serve simple text at root
        app.route("/", get(|| async { "Cratos AI Assistant" }))
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
