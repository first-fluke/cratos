//! Cratos - AI-Powered Personal Assistant
//!
//! CLI entry point for the Cratos server.

#![forbid(unsafe_code)]

use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod acp;
mod api;
mod cli;
mod middleware;
mod server;
mod websocket;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let is_tui = std::env::args().any(|a| a == "tui");

    let data_dir = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .map(|p| p.join(".cratos"))
        .unwrap_or_else(|| std::path::PathBuf::from(".cratos"));
    let _ = std::fs::create_dir_all(&data_dir);

    let (non_blocking, _guard) = if is_tui {
        let file_appender = tracing_appender::rolling::never(&data_dir, "tui.log");
        let (nb, guard) = tracing_appender::non_blocking(file_appender);
        (nb, Some(guard))
    } else {
        let (nb, guard) = tracing_appender::non_blocking(std::io::stderr());
        (nb, Some(guard))
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cratos=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
        .init();

    let cli = cli::Cli::parse();

    // init creates .env, so skip startup log and .env warning for it
    let skip_startup_log = matches!(
        &cli.command,
        Some(cli::Commands::Init { .. })
            | Some(cli::Commands::Serve)
            | Some(cli::Commands::Tui { .. })
            | Some(cli::Commands::Acp { .. })
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
