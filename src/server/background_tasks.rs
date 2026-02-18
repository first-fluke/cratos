//! Background task startup functions
//!
//! Contains functions to start scheduler, skill generation, and cleanup tasks.

use super::config::AppConfig;
use cratos_core::{EventBus, Orchestrator, SchedulerConfig, SchedulerEngine, SchedulerStore, ShutdownController};
use cratos_replay::EventStore;
use cratos_skills::{SkillRegistry, SkillStore};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Start the proactive scheduler
pub async fn start_scheduler(
    config: &AppConfig,
    data_dir: &Path,
    orchestrator: &Arc<Orchestrator>,
    event_bus: &Arc<EventBus>,
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

            // Build real executor using orchestrator and event bus
            let sched_orch = orchestrator.clone();
            let sched_event_bus = event_bus.clone();
            let task_executor: cratos_core::scheduler::TaskExecutor = Arc::new(
                move |action: cratos_core::scheduler::TaskAction| {
                    let orch = sched_orch.clone();
                    let eb = sched_event_bus.clone();
                    Box::pin(async move {
                        crate::server::task_handler::execute_task(action, orch, eb).await
                    })
                },
            );

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

/// Start the skill generation background task
pub fn start_skill_generation_task(
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

/// Start the cleanup background task
pub fn start_cleanup_task(
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
