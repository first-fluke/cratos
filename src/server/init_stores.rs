//! Store initialization functions
//!
//! Contains functions for initializing various data stores used by the server.

use anyhow::{Context, Result};
use cratos_core::chronicles::ChronicleStore;
use cratos_replay::EventStore;
use cratos_skills::{PersonaSkillStore, SkillRegistry, SkillStore};
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

/// Result of store initialization
pub struct StoreBundle {
    pub event_store: Arc<EventStore>,
    pub chronicle_store: Arc<ChronicleStore>,
    pub skill_store: Arc<SkillStore>,
    pub skill_registry: Arc<SkillRegistry>,
    pub persona_skill_store: Arc<PersonaSkillStore>,
}

/// Initialize all data stores
pub async fn init_stores(data_dir: &Path) -> Result<StoreBundle> {
    // Event store (SQLite)
    let db_path = data_dir.join("cratos.db");
    let event_store = Arc::new(
        EventStore::from_path(&db_path)
            .await
            .context("Failed to initialize SQLite event store")?,
    );
    info!("SQLite event store initialized at {}", db_path.display());

    // Chronicle store
    let chronicle_store = Arc::new(ChronicleStore::with_path(data_dir));
    info!("Chronicle store initialized at {}", data_dir.display());

    // Skill store (SQLite)
    let skill_db_path = data_dir.join("skills.db");
    let skill_store = Arc::new(
        SkillStore::from_path(&skill_db_path)
            .await
            .context("Failed to initialize SQLite skill store")?,
    );
    info!(
        "SQLite skill store initialized at {}",
        skill_db_path.display()
    );

    // Skill registry
    let skill_registry = Arc::new(SkillRegistry::new());
    let active_skills = skill_store.list_active_skills().await.unwrap_or_default();
    for skill in active_skills {
        if let Err(e) = skill_registry.register(skill).await {
            warn!("Failed to register skill: {}", e);
        }
    }
    let skill_count = skill_registry.count().await;
    info!(
        "Skill registry initialized with {} active skills",
        skill_count
    );

    // Persona-skill store (same DB as skill_store)
    let persona_skill_store = Arc::new(
        PersonaSkillStore::from_path(&skill_db_path)
            .await
            .context("Failed to initialize persona skill store")?,
    );
    info!("Persona skill store initialized");

    Ok(StoreBundle {
        event_store,
        chronicle_store,
        skill_store,
        skill_registry,
        persona_skill_store,
    })
}
