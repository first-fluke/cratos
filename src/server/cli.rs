//! CLI orchestrator builder
//!
//! Creates a minimal orchestrator for CLI commands without starting the full server.

use super::config::AppConfig;
use super::providers::resolve_llm_provider;
use anyhow::{Context, Result};
use cratos_core::{Orchestrator, OrchestratorConfig, PlannerConfig};
use cratos_llm::LlmProvider;
use cratos_replay::EventStore;
use cratos_tools::{
    register_builtins_with_config, BuiltinsConfig, ExecConfig, ExecMode, RunnerConfig, ToolRegistry,
};
use std::sync::Arc;

/// Build a minimal orchestrator for CLI commands (e.g., `cratos develop`).
///
/// Does NOT start the server, channels, scheduler, or background tasks.
/// Initialises: config → LLM → tools → event store → orchestrator.
pub async fn build_orchestrator_for_cli(config: &AppConfig) -> Result<Arc<Orchestrator>> {
    let data_dir = config
        .data_dir
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(cratos_replay::default_data_dir);

    let db_path = data_dir.join("cratos.db");
    let event_store = Arc::new(
        EventStore::from_path(&db_path)
            .await
            .context("Failed to initialize SQLite event store")?,
    );

    let llm_router = resolve_llm_provider(&config.llm)?;
    let llm_provider: Arc<dyn LlmProvider> = llm_router.clone();

    let mut tool_registry = ToolRegistry::new();
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
    let tool_registry = Arc::new(tool_registry);

    let exec_timeout = std::time::Duration::from_secs(config.security.exec.max_timeout_secs);
    let allow_high_risk = config.approval.default_mode == "never";
    let runner_config = RunnerConfig::new(exec_timeout).with_high_risk(allow_high_risk);

    let orchestrator_config = OrchestratorConfig::new()
        .with_max_iterations(15)
        .with_logging(true)
        .with_planner_config({
            let (prov_name, model_name) = if llm_provider.name() == "router" {
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

    let mut orchestrator = Orchestrator::new(llm_provider, tool_registry, orchestrator_config)
        .with_event_store(event_store)
        .with_persona_mapping(cratos_core::PersonaMapping::default_mapping());

    // Auto-detect fallback provider
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
            orchestrator = orchestrator.with_fallback_provider(fb);
        }
    }

    Ok(Arc::new(orchestrator))
}
