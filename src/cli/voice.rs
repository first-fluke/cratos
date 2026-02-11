//! Voice control CLI command
//!
//! `cratos voice` - Start interactive voice assistant
//!
//! Listens for voice activity, transcribes speech, processes with Orchestrator,
//! and speaks the response back using TTS.

use anyhow::{Context, Result};
use cratos_audio::{VoiceConfig, VoiceController, VoiceEvent};
use cratos_core::{ApprovalManager, Orchestrator, OrchestratorConfig, PlannerConfig};
use cratos_replay::EventStore;
use cratos_tools::{register_builtins, RunnerConfig, ToolRegistry};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

/// Run voice assistant.
pub async fn run(language: Option<String>) -> Result<()> {
    // ── Orchestrator bootstrap (same as TUI) ─────────────────────
    let config = crate::server::load_config().context("Failed to load configuration")?;
    let llm_provider: Arc<dyn cratos_llm::LlmProvider> =
        crate::server::resolve_llm_provider(&config.llm)?;

    info!("Voice: LLM provider = {}", llm_provider.name());

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
        .with_planner_config(
            PlannerConfig::default()
                .with_machine_info()
                .with_provider_info(llm_provider.name(), llm_provider.default_model()),
        )
        .with_runner_config(RunnerConfig::default());

    let orchestrator = Arc::new(
        Orchestrator::new(llm_provider, tool_registry, orch_config)
            .with_event_store(event_store)
            .with_memory(session_store)
            .with_approval_manager(Arc::new(ApprovalManager::new())),
    );

    // ── Voice config ─────────────────────────────────────────────
    let mut voice_config = VoiceConfig::default();
    if let Some(lang) = language {
        voice_config = voice_config.with_language(lang);
    }

    let controller = VoiceController::new(voice_config)?;

    println!("\nCratos Voice Assistant");
    println!("{}", "-".repeat(40));
    println!("  Mode:      {:?}", controller.mode());
    println!("  STT:       {}", if controller.stt_enabled() { "enabled" } else { "disabled" });
    println!("  Language:  {}", controller.config().language);
    println!("  Wake word: {}", controller.config().wake_word.name);
    println!();

    if !controller.stt_enabled() {
        println!("Voice recognition is not available.");
        println!("To enable: export OPENAI_API_KEY=\"sk-...\"");
        println!("Or compile with --features local-stt for offline mode.");
        return Ok(());
    }

    println!("Listening... Say '{}' to activate.", controller.config().wake_word.name);
    println!("Press Ctrl+C to exit.\n");

    // ── Event handler ────────────────────────────────────────────
    let (event_tx, mut event_rx) = mpsc::channel::<VoiceEvent>(32);

    let event_handle = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                VoiceEvent::WakeWordDetected => println!("[*] Wake word detected"),
                VoiceEvent::Listening => println!("[>] Listening..."),
                VoiceEvent::StoppedListening => println!("[.] Processing..."),
                VoiceEvent::Transcribed(text) => println!("[T] You: {text}"),
                VoiceEvent::Speaking => {} // silent
                VoiceEvent::SpeakingFinished => println!("[<] Ready\n"),
                VoiceEvent::Error(e) => eprintln!("[!] Error: {e}"),
            }
        }
    });

    // ── Run interactive loop ─────────────────────────────────────
    controller
        .run_interactive(orchestrator, Some(event_tx))
        .await?;

    event_handle.abort();
    Ok(())
}
