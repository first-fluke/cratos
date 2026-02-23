//! CLI configuration management
//!
//! Provides `cratos config {list,get,set,reset,edit}` commands
//! for viewing and modifying settings via config/local.toml.

use crate::server::config::ConfigValidator;
use crate::server::{load_config, DEFAULT_CONFIG};
use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::Path;

const LOCAL_CONFIG_PATH: &str = "config/local.toml";

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// List all settings (grouped by category)
    List {
        /// Filter by category (general, llm, channels, security, tools, advanced)
        #[arg(short, long)]
        category: Option<String>,
    },
    /// Get a specific setting value
    Get {
        /// Setting key (dot notation: llm.default_provider)
        key: String,
    },
    /// Set a setting value
    Set {
        /// Setting key (dot notation)
        key: String,
        /// New value
        value: String,
    },
    /// Reset a setting to default
    Reset {
        /// Setting key (or "all" for full reset)
        key: String,
    },
    /// Open config file in $EDITOR
    Edit,
}

pub fn run(cmd: ConfigCommands) -> Result<()> {
    match cmd {
        ConfigCommands::List { category } => cmd_list(category.as_deref()),
        ConfigCommands::Get { key } => cmd_get(&key),
        ConfigCommands::Set { key, value } => cmd_set(&key, &value),
        ConfigCommands::Reset { key } => cmd_reset(&key),
        ConfigCommands::Edit => cmd_edit(),
    }
}

// ── list ──

fn cmd_list(category_filter: Option<&str>) -> Result<()> {
    let config = load_config().context("Failed to load configuration")?;
    let view = crate::api::config::AppConfigView::from(config);

    let categories: Vec<(&str, Vec<(&str, String)>)> = vec![
        (
            "General",
            vec![
                ("general.language", view.general.language.clone()),
                ("general.persona", view.general.persona.clone()),
            ],
        ),
        (
            "LLM",
            {
                let mut items = vec![
                    (
                        "llm.default_provider",
                        view.llm.default_provider.clone(),
                    ),
                    ("llm.model", view.llm.model.clone()),
                ];
                if let Some(ref mr) = view.llm.model_routing {
                    if let Some(ref r) = mr.simple {
                        items.push((
                            "llm.model_routing.simple",
                            format!("{}/{}", r.provider, r.model),
                        ));
                    }
                    if let Some(ref r) = mr.general {
                        items.push((
                            "llm.model_routing.general",
                            format!("{}/{}", r.provider, r.model),
                        ));
                    }
                    if let Some(ref r) = mr.complex {
                        items.push((
                            "llm.model_routing.complex",
                            format!("{}/{}", r.provider, r.model),
                        ));
                    }
                    if let Some(ref r) = mr.fallback {
                        items.push((
                            "llm.model_routing.fallback",
                            format!("{}/{}", r.provider, r.model),
                        ));
                    }
                    items.push((
                        "llm.model_routing.auto_downgrade",
                        mr.auto_downgrade.to_string(),
                    ));
                }
                items
            },
        ),
        (
            "Channels",
            vec![
                (
                    "channels.telegram_enabled",
                    view.channels.telegram_enabled.to_string(),
                ),
                (
                    "channels.slack_enabled",
                    view.channels.slack_enabled.to_string(),
                ),
                (
                    "channels.discord_enabled",
                    view.channels.discord_enabled.to_string(),
                ),
            ],
        ),
        (
            "Security",
            vec![
                (
                    "security.approval_mode",
                    view.security.approval_mode.clone(),
                ),
                (
                    "security.sandbox_policy",
                    view.security.sandbox_policy.clone(),
                ),
                ("security.exec_mode", view.security.exec_mode.clone()),
                (
                    "security.injection_protection",
                    view.security.injection_protection.to_string(),
                ),
            ],
        ),
        (
            "Tools",
            vec![
                (
                    "tools.scheduler_enabled",
                    view.tools.scheduler_enabled.to_string(),
                ),
                (
                    "tools.scheduler_check_interval_secs",
                    view.tools.scheduler_check_interval_secs.to_string(),
                ),
                (
                    "tools.vector_search_enabled",
                    view.tools.vector_search_enabled.to_string(),
                ),
                (
                    "tools.browser_enabled",
                    view.tools.browser_enabled.to_string(),
                ),
                ("tools.mcp_enabled", view.tools.mcp_enabled.to_string()),
            ],
        ),
        (
            "Advanced",
            vec![
                (
                    "advanced.server_port",
                    view.advanced.server_port.to_string(),
                ),
                (
                    "advanced.replay_retention_days",
                    view.advanced.replay_retention_days.to_string(),
                ),
                ("advanced.redis_url", view.advanced.redis_url.clone()),
            ],
        ),
    ];

    let local_exists = Path::new(LOCAL_CONFIG_PATH).exists();

    for (cat_name, items) in &categories {
        if let Some(filter) = category_filter {
            if !cat_name.eq_ignore_ascii_case(filter) {
                continue;
            }
        }
        println!();
        println!("  Category: {}", cat_name);
        println!("  {}", "\u{2500}".repeat(50));
        for (key, value) in items {
            let marker = if local_exists && is_overridden(key) {
                " *"
            } else {
                ""
            };
            println!("  {:<42} {}{}", key, value, marker);
        }
    }

    if local_exists {
        println!();
        println!("  (* = overridden in config/local.toml)");
    }
    println!();

    Ok(())
}

/// Check if a key has been overridden in local config
fn is_overridden(key: &str) -> bool {
    let local_path = Path::new(LOCAL_CONFIG_PATH);
    if !local_path.exists() {
        return false;
    }
    let content = match std::fs::read_to_string(local_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let doc: toml_edit::DocumentMut = match content.parse() {
        Ok(d) => d,
        Err(_) => return false,
    };
    resolve_toml_path(&doc, key).is_some()
}

/// Resolve a dot-notation key in a TOML document
fn resolve_toml_path<'a>(doc: &'a toml_edit::DocumentMut, key: &str) -> Option<&'a toml_edit::Item> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current: &toml_edit::Item = doc.as_item();
    for part in &parts {
        current = current.get(part)?;
    }
    if current.is_none() {
        None
    } else {
        Some(current)
    }
}

// ── get ──

fn cmd_get(key: &str) -> Result<()> {
    let config = load_config().context("Failed to load configuration")?;
    let view = crate::api::config::AppConfigView::from(config);
    let json = serde_json::to_value(&view).context("Failed to serialize config")?;

    // Navigate dot-notation path through JSON
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = &json;
    for part in &parts {
        match current.get(part) {
            Some(v) => current = v,
            None => {
                eprintln!("Key not found: {}", key);
                std::process::exit(1);
            }
        }
    }

    match current {
        serde_json::Value::String(s) => println!("{}", s),
        serde_json::Value::Bool(b) => println!("{}", b),
        serde_json::Value::Number(n) => println!("{}", n),
        other => println!("{}", serde_json::to_string_pretty(other)?),
    }

    Ok(())
}

// ── set ──

fn cmd_set(key: &str, value: &str) -> Result<()> {
    // Validate the value before writing
    validate_key_value(key, value)?;

    // Map view keys to TOML paths
    let toml_path = view_key_to_toml_path(key);

    // Read or create local config
    let local_path = Path::new(LOCAL_CONFIG_PATH);
    let content = if local_path.exists() {
        std::fs::read_to_string(local_path)?
    } else {
        String::new()
    };

    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .context("Failed to parse config/local.toml")?;

    // Set the value
    set_toml_value(&mut doc, &toml_path, value)?;

    // Ensure parent directory exists
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(local_path, doc.to_string())?;

    println!("Set {} = {}", key, value);

    // Check if restart is needed
    if requires_restart(key) {
        println!("Note: This setting requires a server restart to take effect.");
    }

    Ok(())
}

fn validate_key_value(key: &str, value: &str) -> Result<()> {
    match key {
        "general.language" | "language" => {
            ConfigValidator::validate_language(value).map_err(|e| anyhow::anyhow!(e))
        }
        "general.persona" | "persona" => {
            ConfigValidator::validate_persona(value).map_err(|e| anyhow::anyhow!(e))
        }
        "llm.default_provider" => {
            ConfigValidator::validate_provider(value).map_err(|e| anyhow::anyhow!(e))
        }
        "security.approval_mode" => {
            ConfigValidator::validate_approval_mode(value).map_err(|e| anyhow::anyhow!(e))
        }
        "security.exec_mode" => {
            ConfigValidator::validate_exec_mode(value).map_err(|e| anyhow::anyhow!(e))
        }
        "advanced.server_port" => {
            let port: u16 = value
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid port number"))?;
            ConfigValidator::validate_port(port).map_err(|e| anyhow::anyhow!(e))
        }
        _ => Ok(()), // No validation for unknown keys
    }
}

/// Map view dot-notation keys to actual TOML config paths
fn view_key_to_toml_path(key: &str) -> String {
    match key {
        "general.language" => "language".to_string(),
        "general.persona" => "persona".to_string(),
        "llm.default_provider" => "llm.default_provider".to_string(),
        "llm.model" => {
            // Model is provider-specific; use a general approach
            "llm.default_model".to_string()
        }
        "security.approval_mode" => "approval.default_mode".to_string(),
        "security.sandbox_policy" => "security.sandbox_policy".to_string(),
        "security.exec_mode" => "security.exec.mode".to_string(),
        "security.injection_protection" => "security.enable_injection_protection".to_string(),
        "channels.telegram_enabled" => "channels.telegram.enabled".to_string(),
        "channels.slack_enabled" => "channels.slack.enabled".to_string(),
        "channels.discord_enabled" => "channels.discord.enabled".to_string(),
        "tools.scheduler_enabled" => "scheduler.enabled".to_string(),
        "tools.scheduler_check_interval_secs" => "scheduler.check_interval_secs".to_string(),
        "tools.vector_search_enabled" => "vector_search.enabled".to_string(),
        "advanced.server_port" => "server.port".to_string(),
        "advanced.replay_retention_days" => "replay.retention_days".to_string(),
        "advanced.redis_url" => "redis.url".to_string(),
        other => other.to_string(),
    }
}

fn set_toml_value(doc: &mut toml_edit::DocumentMut, path: &str, value: &str) -> Result<()> {
    let parts: Vec<&str> = path.split('.').collect();

    // Ensure all intermediate tables exist
    let mut current = doc.as_item_mut();
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Last part: set the value
            let toml_value = parse_toml_value(value);
            current[part] = toml_value;
        } else {
            // Intermediate: ensure table exists
            if !current.get(part).map_or(false, |v| v.is_table()) {
                current[part] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            current = &mut current[part];
        }
    }
    Ok(())
}

fn parse_toml_value(value: &str) -> toml_edit::Item {
    // Try boolean
    if value == "true" {
        return toml_edit::value(true);
    }
    if value == "false" {
        return toml_edit::value(false);
    }
    // Try integer
    if let Ok(n) = value.parse::<i64>() {
        return toml_edit::value(n);
    }
    // Try float
    if let Ok(f) = value.parse::<f64>() {
        if value.contains('.') {
            return toml_edit::value(f);
        }
    }
    // String
    toml_edit::value(value)
}

fn requires_restart(key: &str) -> bool {
    matches!(
        key,
        "llm.default_provider"
            | "llm.model"
            | "channels.telegram_enabled"
            | "channels.slack_enabled"
            | "channels.discord_enabled"
            | "advanced.server_port"
            | "advanced.redis_url"
    )
}

// ── reset ──

fn cmd_reset(key: &str) -> Result<()> {
    let local_path = Path::new(LOCAL_CONFIG_PATH);
    if !local_path.exists() {
        println!("No local overrides found (config/local.toml does not exist).");
        return Ok(());
    }

    if key == "all" {
        std::fs::remove_file(local_path)?;
        println!("All local overrides removed. Using defaults from config/default.toml.");
        return Ok(());
    }

    let toml_path = view_key_to_toml_path(key);
    let content = std::fs::read_to_string(local_path)?;
    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .context("Failed to parse config/local.toml")?;

    let parts: Vec<&str> = toml_path.split('.').collect();
    if parts.len() == 1 {
        doc.remove(parts[0]);
    } else if parts.len() == 2 {
        if let Some(table) = doc.get_mut(parts[0]).and_then(|v| v.as_table_mut()) {
            table.remove(parts[1]);
            if table.is_empty() {
                doc.remove(parts[0]);
            }
        }
    } else if parts.len() == 3 {
        if let Some(t1) = doc.get_mut(parts[0]).and_then(|v| v.as_table_mut()) {
            if let Some(t2) = t1.get_mut(parts[1]).and_then(|v| v.as_table_mut()) {
                t2.remove(parts[2]);
                if t2.is_empty() {
                    t1.remove(parts[1]);
                }
            }
            if t1.is_empty() {
                doc.remove(parts[0]);
            }
        }
    }

    std::fs::write(local_path, doc.to_string())?;

    // Verify the default value
    let _ = std::env::remove_var("_"); // Force reload (no-op, config is file-based)
    let default_doc: toml_edit::DocumentMut = DEFAULT_CONFIG.parse()?;
    let default_value = resolve_toml_path(&default_doc, &toml_path);
    if let Some(val) = default_value {
        println!("Reset {} (default: {})", key, val.to_string().trim());
    } else {
        println!("Reset {} (removed from local config)", key);
    }

    Ok(())
}

// ── edit ──

fn cmd_edit() -> Result<()> {
    let local_path = Path::new(LOCAL_CONFIG_PATH);

    // Create the file with helpful comments if it doesn't exist
    if !local_path.exists() {
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            local_path,
            "# Cratos Local Configuration Overrides\n\
             # Values here override config/default.toml\n\
             # See config/default.toml for available settings\n\n",
        )?;
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") {
            "nano".to_string()
        } else {
            "vi".to_string()
        }
    });

    let status = std::process::Command::new(&editor)
        .arg(LOCAL_CONFIG_PATH)
        .status()
        .context(format!("Failed to launch editor: {}", editor))?;

    if !status.success() {
        eprintln!("Editor exited with non-zero status");
    }

    Ok(())
}
