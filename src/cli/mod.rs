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

pub mod browser_ext;
pub mod chronicle;
pub mod data;
pub mod decrees;
pub mod develop;
pub mod doctor;
pub mod pair;
pub mod pantheon;
pub mod quota;
pub mod security;
pub mod setup;
pub mod skill;
pub mod tui;
pub mod voice;

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
    /// Manage skills (list, show, enable, disable)
    #[command(subcommand)]
    Skill(SkillCommands),
    /// Manage stored data (stats, clear)
    #[command(subcommand)]
    Data(DataCommands),
    /// Start ACP bridge (stdin/stdout JSON-lines for IDE integration)
    Acp {
        /// Auth token (optional, defaults to localhost trust if auth disabled)
        #[arg(long)]
        token: Option<String>,
        /// Use MCP (Model Context Protocol) JSON-RPC 2.0 mode instead of ACP
        #[arg(long)]
        mcp: bool,
    },
    /// Security audit and diagnostics
    #[command(subcommand)]
    Security(SecurityCommands),
    /// Start interactive voice assistant
    Voice {
        /// Language code (ko, en, ja, zh)
        #[arg(short, long)]
        lang: Option<String>,
    },
    /// Browser extension and browser control
    #[command(subcommand)]
    Browser(BrowserCommands),
    /// Device pairing (PIN-based)
    #[command(subcommand)]
    Pair(PairCommands),
    /// Remote development (Issue → PR automation)
    Develop {
        /// GitHub issue URL or number (e.g., "https://github.com/user/repo/issues/123" or "#123")
        issue: String,
        /// Repository URL to clone (if not in a git repo already)
        #[arg(long)]
        repo: Option<String>,
        /// Dry-run mode: show plan without executing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum SecurityCommands {
    /// Run security audit on current configuration
    Audit {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BrowserCommands {
    /// Manage Chrome extension
    #[command(subcommand)]
    Extension(BrowserExtCommands),
    /// List open browser tabs
    Tabs,
    /// Open URL in browser
    Open {
        /// URL to open
        url: String,
    },
    /// Take browser screenshot
    Screenshot {
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
        /// CSS selector to screenshot a specific element
        #[arg(short, long)]
        selector: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum BrowserExtCommands {
    /// Install Chrome extension to ~/.cratos/extensions/chrome
    Install,
    /// Print extension install path
    Path,
}

/// Device pairing subcommands
#[derive(Subcommand, Debug)]
pub enum PairCommands {
    /// Generate a PIN for device pairing
    Start,
    /// List paired devices
    Devices,
    /// Unpair a device
    Unpair {
        /// Device ID to unpair
        device_id: String,
    },
}

/// Data management subcommands
#[derive(Subcommand, Debug)]
pub enum DataCommands {
    /// Show data statistics (record counts, file sizes)
    Stats,
    /// Clear data
    Clear {
        /// Target to clear (omit for all)
        #[command(subcommand)]
        target: Option<ClearTarget>,
        /// Skip confirmation prompt
        #[arg(short, long, global = true)]
        force: bool,
    },
}

/// Specific data targets for clearing
#[derive(Subcommand, Debug)]
pub enum ClearTarget {
    /// Redis sessions
    Sessions,
    /// Graph RAG memory (entities, turns)
    Memory,
    /// Execution history
    History {
        /// Delete records older than N days (0 = all)
        #[arg(long, default_value = "0")]
        older_than: u32,
    },
    /// Persona chronicles
    Chronicles {
        /// Specific persona name
        #[arg(long)]
        persona: Option<String>,
    },
    /// Vector search indexes
    Vectors,
    /// Skills and patterns
    Skills,
}

/// Skill subcommands
#[derive(Subcommand, Debug)]
pub enum SkillCommands {
    /// List all skills
    List {
        /// Show only active skills
        #[arg(long)]
        active: bool,
    },
    /// Show skill details
    Show {
        /// Skill name
        name: String,
    },
    /// Enable (activate) a skill
    Enable {
        /// Skill name to enable
        name: String,
    },
    /// Disable a skill
    Disable {
        /// Skill name to disable
        name: String,
    },
    /// Export a skill to a file
    Export {
        /// Skill name to export
        name: String,
        /// Output file path (default: <name>.skill.json)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import a skill from a file
    Import {
        /// File path to import (.skill.json or .skill.bundle.json)
        path: String,
    },
    /// Export all active skills as a bundle
    Bundle {
        /// Bundle name
        #[arg(short, long, default_value = "cratos-skills")]
        name: String,
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Search remote skill registry
    Search {
        /// Search query
        query: String,
        /// Custom registry URL
        #[arg(long)]
        registry: Option<String>,
    },
    /// Install a skill from remote registry
    Install {
        /// Skill name (e.g., "deploy-k8s")
        name: String,
        /// Custom registry URL
        #[arg(long)]
        registry: Option<String>,
    },
    /// Publish a skill to remote registry
    Publish {
        /// Skill name to publish
        name: String,
        /// Registry API token
        #[arg(long)]
        token: Option<String>,
        /// Custom registry URL
        #[arg(long)]
        registry: Option<String>,
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
    /// Clean orphaned chronicles or reset judgment scores
    Clean {
        /// Persona name to remove (e.g., "unknown")
        name: Option<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
        /// Reset all judgment scores (clear accumulated penalties)
        #[arg(long)]
        reset_judgments: bool,
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
        Some(Commands::Skill(cmd)) => skill::run(cmd).await,
        Some(Commands::Data(cmd)) => data::run(cmd).await,
        Some(Commands::Tui { persona }) => tui::run(persona).await,
        Some(Commands::Acp { token, mcp }) => {
            if mcp {
                crate::acp::mcp_compat::run_mcp().await
            } else {
                crate::acp::bridge::run_acp(token).await
            }
        }
        Some(Commands::Security(cmd)) => match cmd {
            SecurityCommands::Audit { json } => security::run_audit_cli(json).await,
        },
        Some(Commands::Voice { lang }) => voice::run(lang).await,
        Some(Commands::Pair(cmd)) => pair::run(cmd).await,
        Some(Commands::Develop {
            issue,
            repo,
            dry_run,
        }) => develop::run(&issue, repo.as_deref(), dry_run).await,
        Some(Commands::Browser(cmd)) => match cmd {
            BrowserCommands::Extension(ext) => match ext {
                BrowserExtCommands::Install => browser_ext::install().await,
                BrowserExtCommands::Path => browser_ext::path().await,
            },
            BrowserCommands::Tabs => browser_ext::tabs().await,
            BrowserCommands::Open { url } => browser_ext::open(&url).await,
            BrowserCommands::Screenshot { output, selector } => {
                browser_ext::screenshot(output.as_deref(), selector.as_deref()).await
            }
        },
        None => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            cmd.print_help()?;
            println!();
            Ok(())
        }
    }
}
