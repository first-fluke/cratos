//! CLI module for Cratos
//!
//! Provides interactive commands:
//! - `init`: Interactive setup wizard for .env configuration
//! - `doctor`: System diagnostics and health checks
//! - `pantheon`: Manage Olympus Pantheon (personas)
//! - `decrees`: View and manage decrees (laws)
//! - `chronicle`: View and manage chronicles (records)

use clap::{Parser, Subcommand, ValueEnum};

pub mod chronicle;
pub mod decrees;
pub mod doctor;
pub mod init;
pub mod pantheon;

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
    /// Manage Olympus Pantheon (personas)
    #[command(subcommand)]
    Pantheon(PantheonCommands),
    /// View and manage decrees (laws)
    #[command(subcommand)]
    Decrees(DecreesCommands),
    /// View and manage chronicles (records)
    #[command(subcommand)]
    Chronicle(ChronicleCommands),
    /// Start the server (default)
    Serve,
}

/// Pantheon subcommands
#[derive(Subcommand, Debug)]
pub enum PantheonCommands {
    /// List all personas
    List,
    /// Show persona details
    Show {
        /// Persona name (e.g., sindri, athena)
        name: String,
    },
    /// Summon (activate) a persona
    Summon {
        /// Persona name to summon
        name: String,
    },
}

/// Decrees subcommands
#[derive(Subcommand, Debug)]
pub enum DecreesCommands {
    /// Show a decree document
    Show {
        /// Decree type to show
        #[arg(value_enum)]
        decree: DecreeType,
    },
    /// Validate decree compliance
    Validate,
}

/// Available decree types
#[derive(Clone, Debug, ValueEnum)]
pub enum DecreeType {
    /// Laws (율법) - 10 articles
    Laws,
    /// Ranks (신격 체계) - Lv1-10
    Ranks,
    /// Warfare rules (개발 규칙)
    Warfare,
}

/// Chronicle subcommands
#[derive(Subcommand, Debug)]
pub enum ChronicleCommands {
    /// List all chronicles
    List,
    /// Show persona's chronicle
    Show {
        /// Persona name
        name: String,
    },
    /// Add log entry to chronicle
    Log {
        /// Log message
        message: String,
        /// Law reference (e.g., "2" for Article 2)
        #[arg(short, long)]
        law: Option<String>,
        /// Persona name (default: active persona)
        #[arg(short, long)]
        persona: Option<String>,
    },
    /// Request promotion for a persona
    Promote {
        /// Persona name to promote
        name: String,
    },
}

/// Run the CLI command
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Some(Commands::Init) => init::run().await,
        Some(Commands::Doctor) => doctor::run().await,
        Some(Commands::Pantheon(cmd)) => pantheon::run(cmd).await,
        Some(Commands::Decrees(cmd)) => decrees::run(cmd).await,
        Some(Commands::Chronicle(cmd)) => chronicle::run(cmd).await,
        Some(Commands::Serve) => crate::server::run().await,
        None => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            cmd.print_help()?;
            println!();
            Ok(())
        }
    }
}
