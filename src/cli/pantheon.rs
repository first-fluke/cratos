//! Pantheon CLI commands
//!
//! `cratos pantheon` - Manage Olympus Pantheon (personas)

use super::{PantheonCommands, PersonaSkillCommands};
use anyhow::{Context, Result};
use cratos_core::pantheon::{ActivePersonaState, PersonaLoader};
use cratos_skills::{default_skill_db_path, PersonaSkillStore, SkillStore};

/// Run pantheon command
pub async fn run(cmd: PantheonCommands) -> Result<()> {
    match cmd {
        PantheonCommands::List => list().await,
        PantheonCommands::Show { name } => show(&name).await,
        PantheonCommands::Summon { name } => summon(&name).await,
        PantheonCommands::Dismiss => dismiss().await,
        PantheonCommands::Skill(skill_cmd) => run_skill_cmd(skill_cmd).await,
    }
}

/// List all personas
async fn list() -> Result<()> {
    println!("\n🏛️  Olympus Pantheon\n");

    let loader = PersonaLoader::new();
    let presets = loader.load_all()?;

    if presets.is_empty() {
        println!("  No personas found.");
        println!("  Create persona files in config/pantheon/*.toml");
        return Ok(());
    }

    // Check active persona
    let active = ActivePersonaState::new()
        .load()
        .unwrap_or(None)
        .map(|n| n.to_lowercase());

    // Supreme (Cratos - Lv255)
    let supreme: Vec<_> = presets.iter().filter(|p| p.level.is_supreme()).collect();
    if !supreme.is_empty() {
        println!("  SUPREME:");
        for preset in supreme {
            let marker = if active.as_deref() == Some(&preset.persona.name.to_lowercase()) {
                " ⚡"
            } else {
                ""
            };
            println!(
                "    {:12} Lv{}  {}{}",
                preset.persona.name.to_lowercase(),
                preset.level.level_display(),
                preset.persona.title,
                marker
            );
        }
        println!();
    }

    // Regular personas
    let regular: Vec<_> = presets.iter().filter(|p| !p.level.is_supreme()).collect();
    if !regular.is_empty() {
        println!("  ROLES:");
        for preset in regular {
            let marker = if active.as_deref() == Some(&preset.persona.name.to_lowercase()) {
                " ⚡"
            } else {
                ""
            };
            println!(
                "    {:12} Lv{}  {} ({:?}){}",
                preset.persona.name.to_lowercase(),
                preset.level.level,
                preset.persona.title,
                preset.persona.domain,
                marker
            );
        }
    }

    println!();
    Ok(())
}

/// Show persona details
async fn show(name: &str) -> Result<()> {
    let loader = PersonaLoader::new();
    let preset = loader.load(name)?;

    println!(
        "\n📜 Persona: {} Lv{}\n",
        preset.persona.name,
        preset.level.level_display()
    );
    println!("  Title:  {}", preset.persona.title);
    println!("  Domain: {:?}", preset.persona.domain);
    println!("  Status: {}", preset.level.title);

    if let Some(desc) = &preset.persona.description {
        println!("\n  Description:");
        println!("    {desc}");
    }

    println!("\n  Traits:");
    println!("    Core:       {}", preset.traits.core);
    println!("    Philosophy: \"{}\"", preset.traits.philosophy);

    if !preset.traits.communication_style.is_empty() {
        println!("\n  Communication Style:");
        for style in &preset.traits.communication_style {
            println!("    - {style}");
        }
    }

    if !preset.principles.rules.is_empty() {
        println!("\n  Principles:");
        for (key, value) in preset.principles.sorted_rules() {
            println!("    {key}. {value}");
        }
    }

    if !preset.skills.default.is_empty() {
        println!("\n  Skills:");
        for skill in &preset.skills.default {
            println!("    - {skill}");
        }
    }

    println!();
    Ok(())
}

/// Summon (activate) a persona
async fn summon(name: &str) -> Result<()> {
    let loader = PersonaLoader::new();
    let preset = loader.load(name)?;

    // Save active persona state
    let state = ActivePersonaState::new();
    state.save(&preset.persona.name)?;

    println!("\n⚡ Summoning {}...\n", preset.persona.name);
    println!(
        "[{} Lv{}] Summoned. How may I assist you?",
        preset.persona.name,
        preset.level.level_display()
    );
    println!();

    Ok(())
}

/// Dismiss (deactivate) the active persona
async fn dismiss() -> Result<()> {
    let state = ActivePersonaState::new();

    match state.load()? {
        Some(name) => {
            state.clear()?;
            println!("\n👋 {} has been dismissed.\n", name);
        }
        None => {
            println!("\n⚠️  No active persona to dismiss.\n");
        }
    }

    Ok(())
}

// =========================================================================
// Persona-Skill Commands
// =========================================================================

/// Run persona skill subcommands
async fn run_skill_cmd(cmd: PersonaSkillCommands) -> Result<()> {
    let db_path = default_skill_db_path();
    let persona_store = PersonaSkillStore::from_path(&db_path)
        .await
        .context("Failed to open persona skill store")?;
    let skill_store = SkillStore::from_path(&db_path)
        .await
        .context("Failed to open skill store")?;

    match cmd {
        PersonaSkillCommands::List {
            persona,
            auto_assigned,
            mastered,
        } => skill_list(&persona_store, &persona, auto_assigned, mastered).await,
        PersonaSkillCommands::Show { persona, skill } => {
            skill_show(&persona_store, &persona, &skill).await
        }
        PersonaSkillCommands::Claim { persona, skill } => {
            skill_claim(&persona_store, &skill_store, &persona, &skill).await
        }
        PersonaSkillCommands::Release { persona, skill } => {
            skill_release(&persona_store, &skill_store, &persona, &skill).await
        }
        PersonaSkillCommands::Leaderboard { skill, limit } => {
            skill_leaderboard(&persona_store, &skill_store, &skill, limit).await
        }
        PersonaSkillCommands::Summary { persona } => skill_summary(&persona_store, &persona).await,
        PersonaSkillCommands::Sync { persona } => skill_sync(&persona_store, &persona).await,
    }
}

/// List skills bound to a persona
async fn skill_list(
    store: &PersonaSkillStore,
    persona: &str,
    auto_assigned: bool,
    mastered: bool,
) -> Result<()> {
    println!("\n🎯 Skills for {}\n", persona);

    let skills = if auto_assigned {
        store.get_auto_assigned_skills(persona).await?
    } else {
        store.get_persona_skills(persona).await?
    };

    let skills: Vec<_> = if mastered {
        skills
            .into_iter()
            .filter(|b| b.usage_count >= 3 && b.success_rate >= 0.8)
            .collect()
    } else {
        skills
    };

    if skills.is_empty() {
        println!("  No skills found.");
        if auto_assigned {
            println!("  (Try without --auto-assigned to see all skills)");
        }
        println!();
        return Ok(());
    }

    println!(
        "  {:<20} {:<12} {:>8} {:>10} {:>8}",
        "SKILL", "OWNERSHIP", "USES", "SUCCESS", "STREAK"
    );
    println!("  {}", "-".repeat(60));

    for binding in skills {
        let ownership_icon = match binding.ownership_type {
            cratos_skills::OwnershipType::Default => "📦",
            cratos_skills::OwnershipType::Claimed => "✋",
            cratos_skills::OwnershipType::AutoAssigned => "⚡",
        };

        let success_pct = (binding.success_rate * 100.0) as u32;
        let success_bar = format!("{}%", success_pct);

        println!(
            "  {:<20} {} {:<10} {:>8} {:>10} {:>8}",
            truncate(&binding.skill_name, 20),
            ownership_icon,
            binding.ownership_type.as_str(),
            binding.usage_count,
            success_bar,
            binding.consecutive_successes
        );
    }

    println!();
    println!("  Legend: 📦 Default  ✋ Claimed  ⚡ Auto-assigned");
    println!();
    Ok(())
}

/// Show persona-skill binding details
async fn skill_show(store: &PersonaSkillStore, persona: &str, skill: &str) -> Result<()> {
    // We need to find by skill name, which requires querying all persona skills
    let skills = store.get_persona_skills(persona).await?;
    let binding = skills
        .iter()
        .find(|b| b.skill_name.eq_ignore_ascii_case(skill));

    match binding {
        Some(b) => {
            println!("\n📊 {} × {}\n", persona, b.skill_name);
            println!(
                "  Ownership:    {} ({})",
                b.ownership_type,
                match b.ownership_type {
                    cratos_skills::OwnershipType::Default => "from TOML",
                    cratos_skills::OwnershipType::Claimed => "manually assigned",
                    cratos_skills::OwnershipType::AutoAssigned => "earned through usage",
                }
            );
            println!("  Usage Count:  {}", b.usage_count);
            println!("  Successes:    {}", b.success_count);
            println!("  Failures:     {}", b.failure_count);
            println!("  Success Rate: {:.1}%", b.success_rate * 100.0);
            println!(
                "  Streak:       {} consecutive successes",
                b.consecutive_successes
            );

            if let Some(avg) = b.avg_duration_ms {
                println!("  Avg Duration: {}ms", avg);
            }

            if let Some(last) = b.last_used_at {
                println!("  Last Used:    {}", last.format("%Y-%m-%d %H:%M"));
            }

            if let Some(assigned) = b.auto_assigned_at {
                println!("  Auto-assigned: {}", assigned.format("%Y-%m-%d %H:%M"));
            }

            println!();
        }
        None => {
            println!("\n⚠️  No binding found for {} × {}\n", persona, skill);
        }
    }

    Ok(())
}

/// Claim a skill for a persona
async fn skill_claim(
    persona_store: &PersonaSkillStore,
    skill_store: &SkillStore,
    persona: &str,
    skill_name: &str,
) -> Result<()> {
    // Find the skill by name
    let skill = skill_store
        .get_skill_by_name(skill_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", skill_name))?;

    let binding = persona_store
        .claim_skill(persona, skill.id, &skill.name)
        .await?;

    println!(
        "\n✅ Claimed skill '{}' for {}\n",
        binding.skill_name, persona
    );
    Ok(())
}

/// Release a skill from a persona
async fn skill_release(
    persona_store: &PersonaSkillStore,
    skill_store: &SkillStore,
    persona: &str,
    skill_name: &str,
) -> Result<()> {
    let skill = skill_store
        .get_skill_by_name(skill_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", skill_name))?;

    let released = persona_store.release_skill(persona, skill.id).await?;

    if released {
        println!("\n✅ Released skill '{}' from {}\n", skill_name, persona);
    } else {
        println!("\n⚠️  No binding found for {} × {}\n", persona, skill_name);
    }

    Ok(())
}

/// Show skill leaderboard
async fn skill_leaderboard(
    persona_store: &PersonaSkillStore,
    skill_store: &SkillStore,
    skill_name: &str,
    limit: usize,
) -> Result<()> {
    let skill = skill_store
        .get_skill_by_name(skill_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", skill_name))?;

    let leaderboard = persona_store.get_skill_leaderboard(skill.id, limit).await?;

    println!("\n🏆 Leaderboard: {}\n", skill.name);

    if leaderboard.is_empty() {
        println!("  No personas have used this skill yet.");
        println!();
        return Ok(());
    }

    println!(
        "  {:<3} {:<15} {:>8} {:>10}",
        "#", "PERSONA", "USES", "SUCCESS"
    );
    println!("  {}", "-".repeat(40));

    for (i, binding) in leaderboard.iter().enumerate() {
        let medal = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "  ",
        };

        println!(
            "  {} {:<15} {:>8} {:>9.1}%",
            medal,
            binding.persona_name,
            binding.usage_count,
            binding.success_rate * 100.0
        );
    }

    println!();
    Ok(())
}

/// Show persona's skill summary
async fn skill_summary(store: &PersonaSkillStore, persona: &str) -> Result<()> {
    let skills = store.get_persona_skills(persona).await?;

    println!("\n📈 Skill Summary: {}\n", persona);

    if skills.is_empty() {
        println!("  No skills bound to this persona.");
        println!();
        return Ok(());
    }

    let total = skills.len();
    let auto_assigned = skills
        .iter()
        .filter(|b| matches!(b.ownership_type, cratos_skills::OwnershipType::AutoAssigned))
        .count();
    let mastered = skills
        .iter()
        .filter(|b| b.usage_count >= 3 && b.success_rate >= 0.8)
        .count();
    let total_uses: u64 = skills.iter().map(|b| b.usage_count).sum();
    let total_successes: u64 = skills.iter().map(|b| b.success_count).sum();
    let overall_rate = if total_uses > 0 {
        total_successes as f64 / total_uses as f64
    } else {
        1.0
    };

    println!("  Total Skills:    {}", total);
    println!("  Auto-assigned:   {}", auto_assigned);
    println!("  Mastered (≥80%): {}", mastered);
    println!("  Total Uses:      {}", total_uses);
    println!("  Overall Success: {:.1}%", overall_rate * 100.0);

    if !skills.is_empty() {
        println!("\n  Top 5 Skills:");
        let top: Vec<_> = skills
            .iter()
            .filter(|b| b.usage_count >= 3)
            .take(5)
            .collect();

        for binding in top {
            println!(
                "    • {} ({:.0}%, {} uses)",
                binding.skill_name,
                binding.success_rate * 100.0,
                binding.usage_count
            );
        }
    }

    println!();
    Ok(())
}

/// Sync skill proficiency to persona's chronicle
async fn skill_sync(store: &PersonaSkillStore, persona: &str) -> Result<()> {
    use cratos_core::{OlympusConfig, OlympusHooks};

    println!("\n🔄 Syncing skill proficiency for {}...\n", persona);

    // Create OlympusHooks to perform the actual sync
    let hooks = OlympusHooks::new(OlympusConfig::default());
    let result = hooks.sync_skill_proficiency(persona, store).await?;

    if result.success {
        println!("  ✅ Chronicle sync completed:");
        println!("    • Skills updated: {}", result.skills_updated);
        println!(
            "    • New auto-assignments recorded: {}",
            result.new_auto_assignments
        );
    } else {
        println!("  ⚠️  No skills found to sync");
    }

    // Display current proficiency map
    let proficiency = store.get_skill_proficiency_map(persona).await?;
    if !proficiency.is_empty() {
        println!("\n  Current Proficiency ({} skills):", proficiency.len());
        for (skill, rate) in &proficiency {
            let bar = "█".repeat((rate * 10.0) as usize);
            let empty = "░".repeat(10 - (rate * 10.0) as usize);
            println!("    • {}: {}{} {:.1}%", skill, bar, empty, rate * 100.0);
        }
    }

    // Display auto-assigned skills
    let auto_assigned = store.get_auto_assigned_skills(persona).await?;
    if !auto_assigned.is_empty() {
        println!("\n  🏆 Auto-assigned Skills ({}):", auto_assigned.len());
        for binding in &auto_assigned {
            println!(
                "    • {} (earned {})",
                binding.skill_name,
                binding
                    .auto_assigned_at
                    .map(|t| t.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );
        }
    }

    println!();
    Ok(())
}

/// Truncate a string to a maximum length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Handle UTF-8 properly
        let mut end = max_len.saturating_sub(2);
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}..", &s[..end])
    }
}
