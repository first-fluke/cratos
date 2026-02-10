//! TUI chat interface for Cratos
//!
//! Provides a full-screen terminal UI using ratatui + crossterm.
//! Initializes the Orchestrator directly (no server required).

pub mod app;
pub mod event;
pub mod ui;

use anyhow::{Context, Result};
use cratos_core::{ApprovalManager, Orchestrator, OrchestratorConfig, PlannerConfig};
use cratos_replay::EventStore;
use cratos_tools::{register_builtins, RunnerConfig, ToolRegistry};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

use app::App;

/// Run the TUI chat interface.
pub async fn run(persona: Option<String>) -> Result<()> {
    // ── Orchestrator bootstrap (reuses server.rs helpers) ───────────

    let config = crate::server::load_config().context("Failed to load configuration")?;
    let llm_provider: std::sync::Arc<dyn cratos_llm::LlmProvider> =
        crate::server::resolve_llm_provider(&config.llm)?;
    let provider_name = {
        let raw = llm_provider.name().to_string();
        // Resolve the actual default provider name (router returns "router")
        let default_name = config.llm.default_provider.clone();
        let normalized = match default_name.as_str() {
            "google" => "gemini",
            "zhipu" | "zhipuai" => "glm",
            other => other,
        };
        let sources = cratos_llm::cli_auth::get_all_auth_sources();
        if let Some(source) = sources.get(normalized) {
            if *source != cratos_llm::cli_auth::AuthSource::ApiKey {
                format!("{} ({})", normalized, source)
            } else {
                raw
            }
        } else {
            raw
        }
    };

    info!("TUI: LLM provider = {}", provider_name);

    let mut tool_registry = ToolRegistry::new();
    register_builtins(&mut tool_registry);
    let tool_registry = Arc::new(tool_registry);

    let data_dir = config
        .data_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(cratos_replay::default_data_dir);

    let db_path = data_dir.join("cratos.db");
    let event_store = Arc::new(
        EventStore::from_path(&db_path)
            .await
            .context("Failed to initialize event store")?,
    );

    let session_store: Arc<dyn cratos_core::SessionStore> =
        Arc::new(cratos_core::MemoryStore::new());

    let orch_config = OrchestratorConfig::new()
        .with_max_iterations(10)
        .with_logging(true)
        .with_planner_config({
            let (prov_name, model_name) = if llm_provider.name() == "router" {
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
        .with_runner_config(RunnerConfig::default());

    let orchestrator = Arc::new(
        Orchestrator::new(llm_provider, tool_registry, orch_config)
            .with_event_store(event_store)
            .with_memory(session_store)
            .with_approval_manager(Arc::new(ApprovalManager::new()))
            .with_persona_mapping(cratos_core::PersonaMapping::default_mapping()),
    );

    // ── Gemini quota poller (if OAuth token available) ─────────────

    let _gemini_quota_shutdown = cratos_llm::start_gemini_quota_poller().await;

    // ── Terminal setup ──────────────────────────────────────────────

    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    let mut app = App::new(orchestrator, provider_name, persona);

    // ── Main loop ───────────────────────────────────────────────────

    let tick_rate = Duration::from_millis(200);

    let run_result: Result<()> = loop {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        if let Err(e) = event::handle_events(&mut app, tick_rate) {
            break Err(e);
        }

        if app.should_quit {
            break Ok(());
        }
    };

    // ── Restore terminal ────────────────────────────────────────────

    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    run_result
}
