//! Pantheon CLI commands
//!
//! `cratos pantheon` - Manage Olympus Pantheon (personas)

use super::PantheonCommands;
use anyhow::Result;
use cratos_core::pantheon::PersonaLoader;

/// Run pantheon command
pub async fn run(cmd: PantheonCommands) -> Result<()> {
    match cmd {
        PantheonCommands::List => list().await,
        PantheonCommands::Show { name } => show(&name).await,
        PantheonCommands::Summon { name } => summon(&name).await,
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

    // Supreme (Cratos - Lv255)
    let supreme: Vec<_> = presets.iter().filter(|p| p.level.is_supreme()).collect();
    if !supreme.is_empty() {
        println!("  SUPREME:");
        for preset in supreme {
            println!(
                "    {:12} Lv{}  {}",
                preset.persona.name.to_lowercase(),
                preset.level.level_display(),
                preset.persona.title
            );
        }
        println!();
    }

    // Regular personas
    let regular: Vec<_> = presets.iter().filter(|p| !p.level.is_supreme()).collect();
    if !regular.is_empty() {
        println!("  ROLES:");
        for preset in regular {
            println!(
                "    {:12} Lv{}  {} ({:?})",
                preset.persona.name.to_lowercase(),
                preset.level.level,
                preset.persona.title,
                preset.persona.domain
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

    println!("\n⚡ Summoning {}...\n", preset.persona.name);
    println!(
        "[{} Lv{}] Summoned. How may I assist you?",
        preset.persona.name,
        preset.level.level_display()
    );
    println!();

    // TODO: Apply persona to actual session
    // Currently only displays information

    Ok(())
}
