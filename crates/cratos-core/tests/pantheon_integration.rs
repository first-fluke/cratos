//! Pantheon Integration Tests
//!
//! Tests loading actual config/pantheon/*.toml files

use cratos_core::pantheon::{Domain, PersonaLevel, PersonaLoader};
use std::path::PathBuf;

fn project_pantheon_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("config/pantheon")
}

#[test]
fn test_load_all_real_personas() {
    let loader = PersonaLoader::with_path(project_pantheon_dir());
    let personas = loader.load_all().expect("Failed to load personas");

    // Verify Core 5 + Extended 9 personas
    assert_eq!(personas.len(), 14, "Should have 14 personas (5 core + 9 extended)");

    // Verify core names
    let names: Vec<_> = personas.iter().map(|p| p.persona.name.as_str()).collect();
    assert!(names.contains(&"Cratos"), "Should have Cratos");
    assert!(names.contains(&"Athena"), "Should have Athena");
    assert!(names.contains(&"Sindri"), "Should have Sindri");
    assert!(names.contains(&"Heimdall"), "Should have Heimdall");
    assert!(names.contains(&"Mimir"), "Should have Mimir");

    // Verify extended names
    assert!(names.contains(&"Odin"), "Should have Odin");
    assert!(names.contains(&"Hestia"), "Should have Hestia");
    assert!(names.contains(&"Norns"), "Should have Norns");
    assert!(names.contains(&"Apollo"), "Should have Apollo");
    assert!(names.contains(&"Freya"), "Should have Freya");
    assert!(names.contains(&"Tyr"), "Should have Tyr");
    assert!(names.contains(&"Nike"), "Should have Nike");
    assert!(names.contains(&"Thor"), "Should have Thor");
    assert!(names.contains(&"Brok"), "Should have Brok");
}

#[test]
fn test_load_cratos() {
    let loader = PersonaLoader::with_path(project_pantheon_dir());
    let cratos = loader.load("cratos").expect("Failed to load Cratos");

    assert_eq!(cratos.persona.name, "Cratos");
    assert_eq!(cratos.persona.title, "God Slayer, The Strongest");
    assert_eq!(cratos.persona.domain, Domain::Orchestrator);
    assert!(cratos.level.is_supreme());
    assert_eq!(cratos.level.level, PersonaLevel::SUPREME_LEVEL);
}

#[test]
fn test_load_athena() {
    let loader = PersonaLoader::with_path(project_pantheon_dir());
    let athena = loader.load("athena").expect("Failed to load Athena");

    assert_eq!(athena.persona.name, "Athena");
    assert_eq!(athena.persona.domain, Domain::Pm);
    assert_eq!(athena.level.level, 3);
    assert!(!athena.level.is_supreme());
}

#[test]
fn test_load_sindri() {
    let loader = PersonaLoader::with_path(project_pantheon_dir());
    let sindri = loader.load("sindri").expect("Failed to load Sindri");

    assert_eq!(sindri.persona.name, "Sindri");
    assert_eq!(sindri.persona.domain, Domain::Dev);
    assert_eq!(sindri.level.level, 1);
    assert!(sindri.skills.default.contains(&"api_builder".to_string()));
}

#[test]
fn test_load_heimdall() {
    let loader = PersonaLoader::with_path(project_pantheon_dir());
    let heimdall = loader.load("heimdall").expect("Failed to load Heimdall");

    assert_eq!(heimdall.persona.name, "Heimdall");
    assert_eq!(heimdall.persona.domain, Domain::Qa);
    assert_eq!(heimdall.level.level, 2);
}

#[test]
fn test_load_mimir() {
    let loader = PersonaLoader::with_path(project_pantheon_dir());
    let mimir = loader.load("mimir").expect("Failed to load Mimir");

    assert_eq!(mimir.persona.name, "Mimir");
    assert_eq!(mimir.persona.domain, Domain::Researcher);
    assert_eq!(mimir.level.level, 4);
}

#[test]
fn test_persona_to_system_prompt() {
    let loader = PersonaLoader::with_path(project_pantheon_dir());
    let sindri = loader.load("sindri").expect("Failed to load Sindri");

    let prompt = sindri.to_system_prompt("TestUser");

    assert!(prompt.contains("Sindri"));
    assert!(prompt.contains("Forge Master"));
    assert!(prompt.contains("DEV"));
    assert!(prompt.contains("TestUser"));
    assert!(prompt.contains("Laws Article 4"));
}

#[test]
fn test_persona_format_response() {
    let loader = PersonaLoader::with_path(project_pantheon_dir());
    let sindri = loader.load("sindri").expect("Failed to load Sindri");

    let response = sindri.format_response("Task completed.", Some("2"));
    assert_eq!(response, "[Sindri Lv1] Per Laws Article 2, Task completed.");

    let cratos = loader.load("cratos").expect("Failed to load Cratos");
    let response = cratos.format_response("Issuing command.", None);
    assert_eq!(response, "[Cratos Lv∞] Issuing command.");
}

#[test]
fn test_persona_to_agent_config() {
    let loader = PersonaLoader::with_path(project_pantheon_dir());
    let athena = loader.load("athena").expect("Failed to load Athena");

    let config = athena.to_agent_config("TestUser");

    assert_eq!(config.id, "athena");
    assert_eq!(config.name, "Athena");
    assert!(config.enabled);
    assert_eq!(config.routing.priority, Domain::Pm.priority());
}

#[test]
fn test_personas_sorted_by_level() {
    let loader = PersonaLoader::with_path(project_pantheon_dir());
    let personas = loader.load_all().expect("Failed to load personas");

    // Should be sorted by level descending (Supreme first)
    let levels: Vec<_> = personas.iter().map(|p| p.level.level).collect();
    for i in 1..levels.len() {
        assert!(
            levels[i - 1] >= levels[i],
            "Personas should be sorted by level descending"
        );
    }

    // First should be Cratos (Supreme)
    assert_eq!(personas[0].persona.name, "Cratos");
    assert!(personas[0].level.is_supreme());
}
