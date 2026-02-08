//! CLI module for Cratos
//!
//! Provides interactive commands:
//! - `init`: Unified interactive setup wizard (i18n, fallback prompts)
//! - `doctor`: System diagnostics and health checks
//! - `pantheon`: Manage Olympus Pantheon (personas)
//! - `decrees`: View and manage decrees (laws)
//! - `chronicle`: View and manage chronicles (records)

use clap::{Parser, Subcommand, ValueEnum};

/// Path to the environment configuration file.
pub const ENV_FILE_PATH: &str = ".env";

pub mod chronicle;
pub mod decrees;
pub mod doctor;
pub mod pantheon;
pub mod quota;
pub mod setup;
pub mod tui;

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
    Init {
        /// Language code (en, ko). Auto-detected if not specified.
        #[arg(short, long)]
        lang: Option<String>,
    },
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
    /// Show provider quota and rate limit status
    Quota {
        /// Output as JSON (for scripting)
        #[arg(long)]
        json: bool,
        /// Live-refresh mode (every 2 seconds)
        #[arg(long)]
        watch: bool,
    },
    /// Start the server (default)
    Serve,
    /// Launch interactive TUI chat
    Tui {
        /// Persona to start with (e.g., sindri, athena)
        #[arg(short, long)]
        persona: Option<String>,
    },
    /// Start ACP bridge (stdin/stdout JSON-lines for IDE integration)
    Acp {
        /// Auth token (optional, defaults to localhost trust if auth disabled)
        #[arg(long)]
        token: Option<String>,
    },
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
    /// Dismiss (deactivate) the active persona
    Dismiss,
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
    /// Alliance (협업 규칙)
    Alliance,
    /// Tribute (보상/비용 규칙)
    Tribute,
    /// Judgment (평가 프레임워크)
    Judgment,
    /// Culture (문화/가치관)
    Culture,
    /// Operations (운영 절차)
    Operations,
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
        Some(Commands::Init { lang }) => setup::run(lang.as_deref()).await,
        Some(Commands::Doctor) => doctor::run().await,
        Some(Commands::Pantheon(cmd)) => pantheon::run(cmd).await,
        Some(Commands::Decrees(cmd)) => decrees::run(cmd).await,
        Some(Commands::Chronicle(cmd)) => chronicle::run(cmd).await,
        Some(Commands::Quota { json, watch }) => quota::run(json, watch).await,
        Some(Commands::Serve) => {
            if !std::path::Path::new(ENV_FILE_PATH).exists() {
                setup::run(None).await?;
                // Reload .env after setup creates it
                let _ = dotenvy::dotenv();
            }
            crate::server::run().await
        }
        Some(Commands::Tui { persona }) => tui::run(persona).await,
        Some(Commands::Acp { token }) => crate::acp::bridge::run_acp(token).await,
        None => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            cmd.print_help()?;
            println!();
            Ok(())
        }
    }
}
