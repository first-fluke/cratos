//! Skill CLI commands
//!
//! `cratos skill` - List, show, enable, and disable skills

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Subcommand;
use cratos_skills::{SkillStatus, SkillStore};

pub mod convert;
pub mod generate;
pub mod list;

#[derive(Subcommand, Debug)]
pub enum SkillCommands {
    /// List available skills
    List {
        /// Show only active skills
        #[arg(long, short)]
        active: bool,
    },
    /// Show skill details
    Show {
        /// Skill name
        name: String,
    },
    /// Enable a skill
    Enable {
        /// Skill name
        name: String,
    },
    /// Disable a skill
    Disable {
        /// Skill name
        name: String,
    },
    /// Export a skill
    Export {
        /// Skill name
        name: String,
        /// Output file path (optional)
        #[arg(short, long)]
        output: Option<String>,
        /// Export as Markdown agent skill (SKILL.md)
        #[arg(long)]
        markdown: bool,
    },
    /// Import a skill
    Import {
        /// File path
        #[arg(help = "Path to the skill file (.json or .skill.json)")]
        path: String,
    },
    /// Create a skill bundle
    Bundle {
        /// Bundle name
        name: String,
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Search remote skill registry
    Search {
        /// Query string
        query: String,
        /// Registry URL (optional)
        #[arg(long)]
        registry: Option<String>,
    },
    /// Install skill from registry
    Install {
        /// Skill name
        name: String,
        /// Registry URL (optional)
        #[arg(long)]
        registry: Option<String>,
    },
    /// Publish skill to registry
    Publish {
        /// Skill name
        name: String,
        /// Registry token
        #[arg(long)]
        token: Option<String>,
        /// Registry URL (optional)
        #[arg(long)]
        registry: Option<String>,
    },
    /// Analyze usage patterns
    Analyze {
        /// Dry run (don't save patterns)
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate skills from patterns
    Generate {
        /// Dry run (don't create skills)
        #[arg(long)]
        dry_run: bool,
        /// Auto-enable generated skills
        #[arg(long)]
        enable: bool,
    },
    /// Prune stale skills
    Prune {
        /// Days without usage
        #[arg(long, default_value = "30")]
        older_than: u32,
        /// Dry run
        #[arg(long)]
        dry_run: bool,
        /// Confirm deletion
        #[arg(long)]
        confirm: bool,
    },
}

/// Status indicator for active skill
pub(crate) const ICON_ACTIVE: &str = "\u{1f7e2}"; // 🟢
/// Status indicator for inactive/disabled/draft skill
pub(crate) const ICON_INACTIVE: &str = "\u{1f534}"; // 🔴

/// Run skill command
pub async fn run(cmd: SkillCommands) -> Result<()> {
    let store = open_store().await?;

    match cmd {
        SkillCommands::List { active } => list::list(&store, active).await,
        SkillCommands::Show { name } => list::show(&store, &name).await,
        SkillCommands::Enable { name } => list::enable(&store, &name).await,
        SkillCommands::Disable { name } => list::disable(&store, &name).await,
        SkillCommands::Export {
            name,
            output,
            markdown,
        } => convert::export_skill(&store, &name, output, markdown).await,
        SkillCommands::Import { path } => convert::import_skill(&store, &path).await,
        SkillCommands::Bundle { name, output } => {
            convert::export_bundle(&store, &name, output).await
        }
        SkillCommands::Search { query, registry } => convert::search_remote(&query, registry).await,
        SkillCommands::Install { name, registry } => {
            convert::install_remote(&store, &name, registry).await
        }
        SkillCommands::Publish {
            name,
            token,
            registry,
        } => convert::publish_remote(&store, &name, token, registry).await,
        SkillCommands::Analyze { dry_run } => generate::analyze_patterns(dry_run).await,
        SkillCommands::Generate { dry_run, enable } => {
            generate::generate_skills(&store, dry_run, enable).await
        }
        SkillCommands::Prune {
            older_than,
            dry_run,
            confirm,
        } => convert::prune(&store, older_than, dry_run, confirm).await,
    }
}

/// Open the default skill store
pub(crate) async fn open_store() -> Result<SkillStore> {
    let db_path = cratos_skills::default_skill_db_path();
    SkillStore::from_path(&db_path)
        .await
        .context("Failed to open skill store")
}

/// Return status icon for a skill status
pub(crate) fn status_icon(status: SkillStatus) -> &'static str {
    match status {
        SkillStatus::Active => ICON_ACTIVE,
        SkillStatus::Disabled | SkillStatus::Draft => ICON_INACTIVE,
    }
}

/// Format success rate as "XX.X% (N/M)"
pub(crate) fn format_rate(rate: f64, usage: u64) -> String {
    if usage == 0 {
        return "-- (0/0)".to_string();
    }
    let successes = (rate * usage as f64).round() as u64;
    format!("{:.1}% ({}/{})", rate * 100.0, successes, usage)
}

/// Format a duration since a timestamp (e.g., "2h ago", "3d ago")
pub(crate) fn format_duration_since(timestamp: chrono::DateTime<Utc>) -> String {
    let elapsed = Utc::now().signed_duration_since(timestamp);
    let secs = elapsed.num_seconds();

    if secs < 0 {
        return "just now".to_string();
    }

    let minutes = secs / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        format!("{days}d ago")
    } else if hours > 0 {
        format!("{hours}h ago")
    } else if minutes > 0 {
        format!("{minutes}m ago")
    } else {
        "just now".to_string()
    }
}

/// Safely truncate a string, respecting UTF-8 char boundaries
pub(crate) fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .take_while(|(i, _)| *i < max_len.saturating_sub(3))
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    format!("{}...", &s[..end])
}
