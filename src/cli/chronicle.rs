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
                println!("  ⏳ {} more entries needed for promotion", remaining);
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
                let remaining = chronicle.entries_until_promotion();
                println!("  ⚠️  Not eligible for promotion.");
                println!("     Need {} more log entries.", remaining);
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
