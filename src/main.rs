//! Cratos - AI-Powered Personal Assistant
//!
//! CLI entry point for the Cratos server.

#![forbid(unsafe_code)]

use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod cli;
mod server;
mod websocket;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cratos=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = cli::Cli::parse();

    // init creates .env, so skip startup log and .env warning for it
    let skip_startup_log = matches!(
        &cli.command,
        Some(cli::Commands::Init { .. }) | Some(cli::Commands::Serve)
    );

    if cli.command.is_some() && !skip_startup_log {
        info!(
            "Starting Cratos AI Assistant v{}",
            env!("CARGO_PKG_VERSION")
        );

        if !std::path::Path::new(cli::ENV_FILE_PATH).exists() {
            warn!(".env file not found. Run 'cratos init' to create one.");
        }
    }

    cli::run(cli).await
}
