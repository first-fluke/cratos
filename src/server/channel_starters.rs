//! Channel adapter startup functions
//!
//! Contains functions to start various channel adapters (Telegram, Slack, Discord, etc.)

use cratos_channels::{
    DiscordAdapter, DiscordConfig, MatrixAdapter, MatrixConfig, TelegramAdapter, TelegramConfig,
    WhatsAppAdapter, WhatsAppConfig,
};
use cratos_core::{DevSessionMonitor, Orchestrator, ShutdownController};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Start the Telegram adapter
pub fn start_telegram_adapter(
    orchestrator: &Arc<Orchestrator>,
    dev_monitor: &Arc<DevSessionMonitor>,
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

/// Start the Slack adapter
pub fn start_slack_adapter(
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
pub fn start_matrix_adapter(
    orchestrator: &Arc<Orchestrator>,
    shutdown_controller: &ShutdownController,
) -> Option<tokio::task::JoinHandle<()>> {
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
pub fn start_discord_adapter(
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
pub fn start_whatsapp_adapter(
    shutdown_controller: &ShutdownController,
) -> Option<tokio::task::JoinHandle<()>> {
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
