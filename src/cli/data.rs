//! Data management CLI commands
//!
//! `cratos data stats`  — show record counts and file sizes
//! `cratos data clear`  — clear data (all or specific targets)

use super::{ClearTarget, DataCommands};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Run a data subcommand.
pub async fn run(cmd: DataCommands) -> Result<()> {
    match cmd {
        DataCommands::Stats => stats().await,
        DataCommands::Clear { target, force } => clear(target, force).await,
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn data_dir() -> PathBuf {
    cratos_replay::default_data_dir()
}

fn file_size_display(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(m) => format_bytes(m.len()),
        Err(_) => "—".to_string(),
    }
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                } else if meta.is_dir() {
                    total += dir_size(&entry.path());
                }
            }
        }
    }
    total
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn confirm(prompt: &str) -> bool {
    use std::io::{self, Write};
    print!("{prompt} [y/N] ");
    io::stdout().flush().ok();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        matches!(input.trim(), "y" | "Y" | "yes" | "YES")
    } else {
        false
    }
}

// ── Stats ────────────────────────────────────────────────────────────

async fn stats() -> Result<()> {
    let dd = data_dir();

    println!("\n📊 Cratos Data Statistics");
    println!("─────────────────────────");

    // Executions (cratos.db)
    let db_path = cratos_replay::default_db_path();
    if db_path.exists() {
        let store = cratos_replay::EventStore::from_path(&db_path).await?;
        // count via a large-limit query
        let execs = store.list_recent_executions(i64::MAX).await.unwrap_or_default();
        println!(
            "Executions:     {:<12}(cratos.db: {})",
            execs.len(),
            file_size_display(&db_path),
        );
    } else {
        println!("Executions:     0           (cratos.db: not found)");
    }

    // Skills (skills.db)
    let skills_path = cratos_skills::default_skill_db_path();
    if skills_path.exists() {
        let store = cratos_skills::SkillStore::from_path(&skills_path).await?;
        let skills = store.list_skills().await.unwrap_or_default();
        println!(
            "Skills:         {:<12}(skills.db: {})",
            skills.len(),
            file_size_display(&skills_path),
        );
    } else {
        println!("Skills:         0           (skills.db: not found)");
    }

    // Memory (memory.db)
    let memory_path = dd.join("memory.db");
    if memory_path.exists() {
        let mem = cratos_memory::GraphMemory::from_path(&memory_path).await?;
        let turns = mem.turn_count().await.unwrap_or(0);
        let entities = mem.entity_count().await.unwrap_or(0);
        println!(
            "Memory turns:   {:<12}(memory.db: {})",
            turns,
            file_size_display(&memory_path),
        );
        println!("Entities:       {}", entities);
    } else {
        println!("Memory turns:   0           (memory.db: not found)");
        println!("Entities:       0");
    }

    // Chronicles
    let chronicles_dir = dd.join("chronicles");
    if chronicles_dir.exists() {
        let store = cratos_core::ChronicleStore::new();
        let chronicles = store.load_all().unwrap_or_default();
        println!("Chronicles:     {} personas", chronicles.len());
    } else {
        println!("Chronicles:     0 personas");
    }

    // Redis sessions
    let redis_count = redis_session_count().await;
    match redis_count {
        Some(n) => println!("Redis sessions: {n}"),
        None => println!("Redis sessions: — (not configured)"),
    }

    // Vectors
    let vectors_dir = cratos_search::default_vectors_dir();
    if vectors_dir.exists() {
        let size = dir_size(&vectors_dir);
        println!(
            "Vectors:        {}           (vectors/: {})",
            if size > 0 { "present" } else { "empty" },
            format_bytes(size),
        );
    } else {
        println!("Vectors:                    (vectors/: not found)");
    }

    println!();
    Ok(())
}

// ── Clear ────────────────────────────────────────────────────────────

async fn clear(target: Option<ClearTarget>, force: bool) -> Result<()> {
    match target {
        None => clear_all(force).await,
        Some(ClearTarget::Sessions) => clear_sessions(force).await,
        Some(ClearTarget::Memory) => clear_memory(force).await,
        Some(ClearTarget::History { older_than }) => clear_history(older_than, force).await,
        Some(ClearTarget::Chronicles { persona }) => clear_chronicles(persona.as_deref(), force).await,
        Some(ClearTarget::Vectors) => clear_vectors(force).await,
        Some(ClearTarget::Skills) => clear_skills(force).await,
    }
}

async fn clear_all(force: bool) -> Result<()> {
    if !force && !confirm("⚠️  This will delete ALL Cratos data. Continue?") {
        println!("Aborted.");
        return Ok(());
    }

    println!("Clearing all data...\n");
    // Order: sessions first (non-critical), then files
    let _ = clear_sessions(true).await;
    let _ = clear_history(0, true).await;
    let _ = clear_skills(true).await;
    let _ = clear_memory(true).await;
    let _ = clear_vectors(true).await;
    let _ = clear_chronicles(None, true).await;

    println!("\n✅ All data cleared.");
    Ok(())
}

async fn clear_sessions(force: bool) -> Result<()> {
    if !force && !confirm("Clear Redis sessions?") {
        println!("Aborted.");
        return Ok(());
    }

    let redis_url = read_redis_url();
    let Some(url) = redis_url else {
        println!("  ℹ️  Redis not configured — skipping sessions.");
        return Ok(());
    };

    match redis::Client::open(url.as_str()) {
        Ok(client) => {
            match client.get_multiplexed_async_connection().await {
                Ok(mut conn) => {
                    // Find and delete cratos:session:* keys
                    let keys: Vec<String> = redis::cmd("KEYS")
                        .arg("cratos:session:*")
                        .query_async(&mut conn)
                        .await
                        .unwrap_or_default();

                    let count = keys.len();
                    for key in &keys {
                        let _: Result<(), _> = redis::cmd("DEL")
                            .arg(key)
                            .query_async(&mut conn)
                            .await;
                    }
                    println!("  ✅ Cleared {count} Redis session(s).");
                }
                Err(e) => println!("  ⚠️  Redis connection failed: {e}"),
            }
        }
        Err(e) => println!("  ⚠️  Invalid Redis URL: {e}"),
    }

    Ok(())
}

async fn clear_memory(force: bool) -> Result<()> {
    if !force && !confirm("Clear Graph RAG memory?") {
        println!("Aborted.");
        return Ok(());
    }

    let path = data_dir().join("memory.db");
    remove_file_if_exists(&path, "memory.db")
}

async fn clear_history(older_than: u32, force: bool) -> Result<()> {
    let label = if older_than == 0 {
        "all execution history".to_string()
    } else {
        format!("execution history older than {older_than} days")
    };

    if !force && !confirm(&format!("Clear {label}?")) {
        println!("Aborted.");
        return Ok(());
    }

    let db_path = cratos_replay::default_db_path();
    if !db_path.exists() {
        println!("  ℹ️  No execution database found.");
        return Ok(());
    }

    let store = cratos_replay::EventStore::from_path(&db_path)
        .await
        .context("Failed to open execution store")?;

    let cutoff = if older_than == 0 {
        chrono::Utc::now() + chrono::Duration::days(1) // future date = delete everything
    } else {
        chrono::Utc::now() - chrono::Duration::days(older_than as i64)
    };

    let deleted = store.delete_old_executions(cutoff).await?;
    println!("  ✅ Deleted {deleted} execution(s).");
    Ok(())
}

async fn clear_chronicles(persona: Option<&str>, force: bool) -> Result<()> {
    let label = match persona {
        Some(p) => format!("chronicles for '{p}'"),
        None => "all chronicles".to_string(),
    };

    if !force && !confirm(&format!("Clear {label}?")) {
        println!("Aborted.");
        return Ok(());
    }

    let chronicles_dir = data_dir().join("chronicles");
    if !chronicles_dir.exists() {
        println!("  ℹ️  No chronicles directory found.");
        return Ok(());
    }

    match persona {
        Some(name) => {
            // Delete files matching this persona name (e.g., sindri_lv*.json)
            let pattern = format!("{}_lv", name.to_lowercase());
            let mut removed = 0;
            if let Ok(entries) = std::fs::read_dir(&chronicles_dir) {
                for entry in entries.flatten() {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if fname.starts_with(&pattern) && fname.ends_with(".json") {
                        std::fs::remove_file(entry.path())?;
                        removed += 1;
                    }
                }
            }
            if removed > 0 {
                println!("  ✅ Removed {removed} chronicle file(s) for '{name}'.");
            } else {
                println!("  ℹ️  No chronicle files found for '{name}'.");
            }
        }
        None => {
            std::fs::remove_dir_all(&chronicles_dir)?;
            println!("  ✅ Removed chronicles directory.");
        }
    }

    Ok(())
}

async fn clear_vectors(force: bool) -> Result<()> {
    if !force && !confirm("Clear vector indexes?") {
        println!("Aborted.");
        return Ok(());
    }

    let vectors_dir = cratos_search::default_vectors_dir();
    if vectors_dir.exists() {
        std::fs::remove_dir_all(&vectors_dir)?;
        println!("  ✅ Removed vectors directory.");
    } else {
        println!("  ℹ️  No vectors directory found.");
    }

    Ok(())
}

async fn clear_skills(force: bool) -> Result<()> {
    if !force && !confirm("Clear skills database?") {
        println!("Aborted.");
        return Ok(());
    }

    let path = cratos_skills::default_skill_db_path();
    remove_file_if_exists(&path, "skills.db")
}

// ── Utilities ────────────────────────────────────────────────────────

fn remove_file_if_exists(path: &Path, label: &str) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
        println!("  ✅ Removed {label}.");
    } else {
        println!("  ℹ️  {label} not found.");
    }
    Ok(())
}

fn read_redis_url() -> Option<String> {
    let env_content = std::fs::read_to_string(".env").unwrap_or_default();
    env_content
        .lines()
        .find(|l| l.starts_with("REDIS_URL=") && !l.starts_with("# "))
        .map(|l| l.trim_start_matches("REDIS_URL=").to_string())
        .filter(|v| !v.is_empty())
}

async fn redis_session_count() -> Option<usize> {
    let url = read_redis_url()?;
    let client = redis::Client::open(url.as_str()).ok()?;
    let mut conn = client.get_multiplexed_async_connection().await.ok()?;
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("cratos:session:*")
        .query_async(&mut conn)
        .await
        .ok()?;
    Some(keys.len())
}
