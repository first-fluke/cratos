//! Skill CLI commands
//!
//! `cratos skill` - List, show, enable, and disable skills

use super::SkillCommands;
use anyhow::{Context, Result};
use chrono::Utc;
use cratos_skills::{SkillStatus, SkillStore};

pub mod convert;
pub mod generate;
pub mod list;

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
