//! Chronicle CLI commands
//!
//! `cratos chronicle` - View and manage chronicles (records)

use super::ChronicleCommands;
use anyhow::Result;
use cratos_core::chronicles::{Chronicle, ChronicleStatus, ChronicleStore};
use cratos_core::pantheon::ActivePersonaState;

/// Run chronicle command
pub async fn run(cmd: ChronicleCommands) -> Result<()> {
    match cmd {
        ChronicleCommands::List => list().await,
        ChronicleCommands::Show { name } => show(&name).await,
        ChronicleCommands::Log {
            message,
            law,
            persona,
        } => log(&message, law.as_deref(), persona.as_deref()).await,
        ChronicleCommands::Promote { name } => promote(&name).await,
        ChronicleCommands::Clean {
            name,
            force,
            reset_judgments,
        } => clean(name.as_deref(), force, reset_judgments).await,
    }
}

/// List all chronicles
async fn list() -> Result<()> {
    println!("\n📚 Chronicles\n");

    let store = ChronicleStore::new();
    let chronicles = store.load_all()?;

    if chronicles.is_empty() {
        println!("  No chronicles found.");
        println!("  Chronicles are created when personas complete tasks.");
        println!();
        println!("  Create a chronicle:");
        println!("    cratos chronicle log \"First task\" --persona sindri");
        println!();
        return Ok(());
    }

    for chronicle in &chronicles {
        let status_icon = match chronicle.status {
            ChronicleStatus::Active => "🟢",
            ChronicleStatus::Inactive => "⚪",
            ChronicleStatus::Promoted => "⬆️",
            ChronicleStatus::Silenced => "🔇",
        };

        let quest_status = if chronicle.quests.is_empty() {
            String::new()
        } else {
            format!(
                " | Quests: {}/{}",
                chronicle.completed_quests(),
                chronicle.quests.len()
            )
        };

        let rating_str = chronicle
            .rating
            .map(|r| format!(" | ★ {r:.1}"))
            .unwrap_or_default();

        println!(
            "  {} {:12} Lv{:<2} | {} entries{}{}",
            status_icon,
            chronicle.persona_name,
            chronicle.level,
            chronicle.log.len(),
            quest_status,
            rating_str
        );
    }

    println!();
    Ok(())
}

/// Show persona's chronicle
async fn show(name: &str) -> Result<()> {
    let store = ChronicleStore::new();

    match store.load(name)? {
        Some(chronicle) => {
            println!(
                "\n📜 Chronicle: {} Lv{}\n",
                chronicle.persona_name, chronicle.level
            );

            // Status
            let status_str = match chronicle.status {
                ChronicleStatus::Active => "Active 🟢",
                ChronicleStatus::Inactive => "Inactive ⚪",
                ChronicleStatus::Promoted => "Promoted ⬆️",
                ChronicleStatus::Silenced => "Silenced 🔇",
            };
            println!("  Status: {status_str}");

            // Objectives
            if !chronicle.objectives.is_empty() {
                println!("\n  Objectives:");
                for obj in &chronicle.objectives {
                    println!("    • {obj}");
                }
            }

            // Quests
            if !chronicle.quests.is_empty() {
                println!("\n  Current Quests:");
                for (i, quest) in chronicle.quests.iter().enumerate() {
                    let mark = if quest.completed { "✓" } else { " " };
                    println!("    [{mark}] {}. {}", i + 1, quest.description);
                }
            }

            // Recent Log
            println!("\n  Recent Log:");
            let recent: Vec<_> = chronicle.log.iter().rev().take(10).collect();
            if recent.is_empty() {
                println!("    (no entries)");
            } else {
                for entry in recent {
                    let law_ref = entry
                        .law_reference
                        .as_ref()
                        .map(|l| format!(" (Art.{l})"))
                        .unwrap_or_default();
                    let commit = entry
                        .commit_hash
                        .as_ref()
                        .map(|h| format!(" [{:.7}]", h))
                        .unwrap_or_default();
                    println!(
                        "    {}  {}{}{}",
                        entry.timestamp.format("%Y-%m-%d"),
                        entry.achievement,
                        law_ref,
                        commit
                    );
                }
            }

            // Judgments
            if !chronicle.judgments.is_empty() {
                println!("\n  Recent Judgments:");
                for judgment in chronicle.judgments.iter().rev().take(3) {
                    let score_str = judgment
                        .score
                        .map(|s| format!(" [{s:.1}/5]"))
                        .unwrap_or_default();
                    println!(
                        "    [{}]{}: \"{}\"",
                        judgment.evaluator, score_str, judgment.comment
                    );
                }
            }

            // Rating
            if let Some(rating) = chronicle.rating {
                println!("\n  Overall Rating: {rating:.1}/5 ★");
            }

            // Promotion status
            println!();
            if chronicle.is_promotion_eligible() {
                println!("  ✅ Eligible for promotion to Lv{}", chronicle.level + 1);
                println!(
                    "     Run: cratos chronicle promote {}",
                    chronicle.persona_name
                );
            } else {
                let remaining = chronicle.entries_until_promotion();
                let rating_gap = chronicle.rating_gap();
                if remaining > 0 {
                    println!("  ⏳ {} more entries needed for promotion", remaining);
                }
                if rating_gap > 0.0 {
                    println!(
                        "  ⏳ Rating {:.1}/5 — need {:.1}+ for promotion (gap: +{:.1})",
                        chronicle.rating.unwrap_or(0.0),
                        3.5,
                        rating_gap,
                    );
                }
            }

            println!();
        }
        None => {
            println!("⚠️  Chronicle not found for: {name}");
            println!();
            println!("  Create a chronicle:");
            println!("    cratos chronicle log \"First task\" --persona {name}");
            println!();
        }
    }

    Ok(())
}

/// Add log entry to chronicle
async fn log(message: &str, law: Option<&str>, persona: Option<&str>) -> Result<()> {
    let active = ActivePersonaState::new().load().unwrap_or(None);
    let persona_name = persona
        .or(active.as_deref())
        .unwrap_or("sindri");

    let store = ChronicleStore::new();
    let mut chronicle = store
        .load(persona_name)?
        .unwrap_or_else(|| Chronicle::new(persona_name));

    chronicle.add_entry(message, law);
    store.save(&chronicle)?;

    println!("✅ Log added to {}'s chronicle", chronicle.persona_name);
    if let Some(law_ref) = law {
        println!("   Referenced: Laws Art.{law_ref}");
    }
    println!(
        "   Total entries: {} (need {} more for promotion)",
        chronicle.log.len(),
        chronicle.entries_until_promotion()
    );
    println!();

    Ok(())
}

/// Request promotion for a persona
async fn promote(name: &str) -> Result<()> {
    println!("\n⬆️  Promotion Request: {name}\n");

    let store = ChronicleStore::new();

    match store.load(name)? {
        Some(mut chronicle) => {
            println!("  Current Level: Lv{}", chronicle.level);
            println!("  Log Entries:   {}", chronicle.log.len());
            println!(
                "  Completed:     {}/{}",
                chronicle.completed_quests(),
                chronicle.quests.len()
            );

            if let Some(rating) = chronicle.rating {
                println!("  Rating:        {rating:.1}/5");
            }

            println!();

            if chronicle.is_promotion_eligible() {
                let old_level = chronicle.level;

                if chronicle.promote() {
                    store.save(&chronicle)?;
                    println!(
                        "  🎉 {} has been promoted from Lv{} to Lv{}!",
                        chronicle.persona_name, old_level, chronicle.level
                    );
                    println!("     New title: {}", get_level_title(chronicle.level));
                } else {
                    println!("  ⚠️  Already at maximum level.");
                }
            } else {
                println!("  ⚠️  Not eligible for promotion.");
                let remaining = chronicle.entries_until_promotion();
                if remaining > 0 {
                    println!("     Need {} more log entries.", remaining);
                }
                let rating_gap = chronicle.rating_gap();
                if rating_gap > 0.0 {
                    println!(
                        "     Rating {:.1}/5 — need 3.5+ (gap: +{:.1})",
                        chronicle.rating.unwrap_or(0.0),
                        rating_gap,
                    );
                }
                println!();
                println!("  Add entries with:");
                println!("    cratos chronicle log \"Work completed\" --persona {name}");
            }
        }
        None => {
            println!("  ⚠️  Chronicle not found for: {name}");
            println!();
            println!("  Create a chronicle first:");
            println!("    cratos chronicle log \"First task\" --persona {name}");
        }
    }

    println!();
    Ok(())
}

/// Clean orphaned chronicles or reset judgment scores
async fn clean(name: Option<&str>, force: bool, reset_judgments: bool) -> Result<()> {
    let store = ChronicleStore::new();

    if let Some(persona) = name {
        // Delete all chronicle files for a specific persona
        let personas = store.list_personas()?;
        if !personas.iter().any(|p| p.eq_ignore_ascii_case(persona)) {
            println!("No chronicles found for \"{persona}\".");
            return Ok(());
        }

        // Find all level files for this persona
        let data_dir = store.data_dir();
        let prefix = persona.to_lowercase();
        let mut files_to_delete = Vec::new();

        if data_dir.exists() {
            for entry in std::fs::read_dir(data_dir)?.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.starts_with(&prefix) && fname.ends_with(".json") {
                    files_to_delete.push(entry.path());
                }
            }
        }

        if files_to_delete.is_empty() {
            println!("No chronicle files found for \"{persona}\".");
            return Ok(());
        }

        println!("\nFound {} file(s) for \"{}\":", files_to_delete.len(), persona);
        for f in &files_to_delete {
            println!("  - {}", f.file_name().unwrap_or_default().to_string_lossy());
        }

        if !force {
            print!("\nDelete these files? [y/N] ");
            use std::io::Write;
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }

        let mut deleted = 0;
        for f in &files_to_delete {
            if std::fs::remove_file(f).is_ok() {
                deleted += 1;
            }
        }
        println!("\nDeleted {deleted} chronicle file(s) for \"{persona}\".");
    }

    if reset_judgments {
        // Reset judgments for all (or specified) persona(s)
        let chronicles = store.load_all()?;
        if chronicles.is_empty() {
            println!("No chronicles found to reset.");
            return Ok(());
        }

        let target_name = name.map(|n| n.to_lowercase());
        let mut reset_count = 0;

        for mut chronicle in chronicles {
            if let Some(ref target) = target_name {
                if chronicle.persona_name.to_lowercase() != *target {
                    continue;
                }
            }

            if chronicle.judgments.is_empty() {
                continue;
            }

            let old_count = chronicle.judgments.len();
            chronicle.judgments.clear();
            chronicle.rating = None;
            store.save(&chronicle)?;
            reset_count += 1;
            println!(
                "  Reset {} judgment(s) for {} Lv{}",
                old_count, chronicle.persona_name, chronicle.level
            );
        }

        if reset_count == 0 {
            println!("No judgments to reset.");
        } else {
            println!("\nReset judgments for {reset_count} persona(s).");
        }
    }

    if name.is_none() && !reset_judgments {
        println!("Usage:");
        println!("  cratos chronicle clean <name> [--force]     Remove orphaned persona chronicles");
        println!("  cratos chronicle clean --reset-judgments     Reset all judgment scores");
        println!("  cratos chronicle clean <name> --reset-judgments  Reset judgments for specific persona");
    }

    println!();
    Ok(())
}

/// Get title for a level
fn get_level_title(level: u8) -> &'static str {
    match level {
        1..=2 => "Mortal",
        3 => "Demigod",
        4 => "Hero",
        5 => "Titan",
        6..=7 => "Lesser God",
        8..=9 => "Olympian",
        10 => "Elder God",
        255 => "Supreme",
        _ => "Unknown",
    }
}
