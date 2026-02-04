//! CLI module for Cratos
//!
//! Provides interactive commands:
//! - `init`: Interactive setup wizard for .env configuration
//! - `doctor`: System diagnostics and health checks

use clap::{Parser, Subcommand};

pub mod doctor;
pub mod init;

/// Cratos AI Assistant CLI
#[derive(Parser, Debug)]
#[command(name = "cratos")]
#[command(about = "AI-Powered Personal Assistant")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Interactive setup wizard
    Init,
    /// Run system diagnostics
    Doctor,
    /// Start the server (default)
    Serve,
}

/// Run the CLI command
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Some(Commands::Init) => init::run().await,
        Some(Commands::Doctor) => doctor::run().await,
        Some(Commands::Serve) => {
            crate::server::run().await
        }
        None => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            cmd.print_help()?;
            println!();
            Ok(())
        }
    }
}
