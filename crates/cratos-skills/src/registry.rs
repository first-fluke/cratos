//! Skill registry for managing available skills.
//!
//! The registry provides in-memory caching and lookup of skills,
//! working alongside the persistent [`SkillStore`](crate::SkillStore).
//!
//! # Overview
//!
//! The [`SkillRegistry`] provides:
//!
//! - **Fast lookup**: O(1) access by ID or name
//! - **Keyword index**: Quick skill discovery by trigger keywords
//! - **Thread-safe**: Uses `Arc<RwLock>` for concurrent access
//!
//! # Example
//!
//! ```ignore
//! use cratos_skills::{SkillRegistry, SkillStore, default_skill_db_path};
//!
//! // Load skills from persistent store
//! let store = SkillStore::from_path(&default_skill_db_path()).await?;
//! let skills = store.list_active_skills().await?;
//!
//! // Create registry and load skills
//! let registry = SkillRegistry::new();
//! let loaded = registry.load_all(skills).await?;
//! println!("Loaded {} skills", loaded);
//!
//! // Lookup by ID or name
//! let skill = registry.get(skill_id).await;
//! let skill = registry.get_by_name("file_reader").await;
//!
//! // Find skills by keyword
//! let matches = registry.get_by_keyword("read").await;
//! ```

use crate::error::{Error, Result};
use crate::skill::{Skill, SkillCategory, SkillOrigin};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// Skill registry for fast in-memory skill lookup
#[derive(Clone)]
pub struct SkillRegistry {
    /// Skills indexed by ID
    skills_by_id: Arc<RwLock<HashMap<Uuid, Arc<Skill>>>>,
    /// Skills indexed by name
    skills_by_name: Arc<RwLock<HashMap<String, Uuid>>>,
    /// Keyword index for fast lookup
    keyword_index: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            skills_by_id: Arc::new(RwLock::new(HashMap::new())),
            skills_by_name: Arc::new(RwLock::new(HashMap::new())),
            keyword_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a skill
    pub async fn register(&self, skill: Skill) -> Result<()> {
        let skill_id = skill.id;
        let skill_name = skill.name.clone();
        let keywords = skill.trigger.keywords.clone();

        // Check for name conflict
        {
            let names = self.skills_by_name.read().await;
            if let Some(existing_id) = names.get(&skill_name) {
                if *existing_id != skill_id {
                    return Err(Error::Validation(format!(
                        "skill with name '{}' already exists",
                        skill_name
                    )));
                }
            }
        }

        // Add to ID index
        {
            let mut skills = self.skills_by_id.write().await;
            skills.insert(skill_id, Arc::new(skill));
        }

        // Add to name index
        {
            let mut names = self.skills_by_name.write().await;
            names.insert(skill_name.clone(), skill_id);
        }

        // Update keyword index
        {
            let mut index = self.keyword_index.write().await;
            for keyword in keywords {
                let keyword_lower = keyword.to_lowercase();
                index.entry(keyword_lower).or_default().push(skill_id);
            }
        }

        debug!("Registered skill: {} ({})", skill_name, skill_id);
        Ok(())
    }

    /// Unregister a skill
    pub async fn unregister(&self, skill_id: Uuid) -> Result<()> {
        // Remove from ID index and get the skill
        let skill = {
            let mut skills = self.skills_by_id.write().await;
            skills.remove(&skill_id)
        };

        let skill = skill.ok_or_else(|| Error::SkillNotFound(skill_id.to_string()))?;

        // Remove from name index
        {
            let mut names = self.skills_by_name.write().await;
            names.remove(&skill.name);
        }

        // Remove from keyword index
        {
            let mut index = self.keyword_index.write().await;
            for keyword in &skill.trigger.keywords {
                let keyword_lower = keyword.to_lowercase();
                if let Some(ids) = index.get_mut(&keyword_lower) {
                    ids.retain(|id| *id != skill_id);
                    if ids.is_empty() {
                        index.remove(&keyword_lower);
                    }
                }
            }
        }

        debug!("Unregistered skill: {} ({})", skill.name, skill_id);
        Ok(())
    }

    /// Get a skill by ID
    pub async fn get(&self, skill_id: Uuid) -> Option<Arc<Skill>> {
        let skills = self.skills_by_id.read().await;
        skills.get(&skill_id).cloned()
    }

    /// Get a skill by name
    pub async fn get_by_name(&self, name: &str) -> Option<Arc<Skill>> {
        let names = self.skills_by_name.read().await;
        if let Some(skill_id) = names.get(name) {
            let skills = self.skills_by_id.read().await;
            skills.get(skill_id).cloned()
        } else {
            None
        }
    }

    /// Get all skills matching a keyword
    pub async fn get_by_keyword(&self, keyword: &str) -> Vec<Arc<Skill>> {
        let keyword_lower = keyword.to_lowercase();
        let index = self.keyword_index.read().await;

        if let Some(skill_ids) = index.get(&keyword_lower) {
            let skills = self.skills_by_id.read().await;
            skill_ids
                .iter()
                .filter_map(|id| skills.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all active skills
    pub async fn get_active(&self) -> Vec<Arc<Skill>> {
        let skills = self.skills_by_id.read().await;
        skills.values().filter(|s| s.is_active()).cloned().collect()
    }

    /// Get all skills
    pub async fn get_all(&self) -> Vec<Arc<Skill>> {
        let skills = self.skills_by_id.read().await;
        skills.values().cloned().collect()
    }

    /// Get skills by category
    pub async fn get_by_category(&self, category: SkillCategory) -> Vec<Arc<Skill>> {
        let skills = self.skills_by_id.read().await;
        skills
            .values()
            .filter(|s| s.category == category)
            .cloned()
            .collect()
    }

    /// Get skills by origin
    pub async fn get_by_origin(&self, origin: SkillOrigin) -> Vec<Arc<Skill>> {
        let skills = self.skills_by_id.read().await;
        skills
            .values()
            .filter(|s| s.origin == origin)
            .cloned()
            .collect()
    }

    /// Get the count of registered skills
    pub async fn count(&self) -> usize {
        let skills = self.skills_by_id.read().await;
        skills.len()
    }

    /// Get the count of active skills
    pub async fn count_active(&self) -> usize {
        let skills = self.skills_by_id.read().await;
        skills.values().filter(|s| s.is_active()).count()
    }

    /// Update a skill
    pub async fn update(&self, skill: Skill) -> Result<()> {
        let skill_id = skill.id;

        // Check if skill exists
        {
            let skills = self.skills_by_id.read().await;
            if !skills.contains_key(&skill_id) {
                return Err(Error::SkillNotFound(skill_id.to_string()));
            }
        }

        // Get old skill for cleanup
        let old_skill = {
            let skills = self.skills_by_id.read().await;
            skills.get(&skill_id).cloned()
        };

        // Clean up old keyword index entries
        if let Some(old) = old_skill {
            let mut index = self.keyword_index.write().await;
            for keyword in &old.trigger.keywords {
                let keyword_lower = keyword.to_lowercase();
                if let Some(ids) = index.get_mut(&keyword_lower) {
                    ids.retain(|id| *id != skill_id);
                    if ids.is_empty() {
                        index.remove(&keyword_lower);
                    }
                }
            }
        }

        // Update name index if name changed
        let new_name = skill.name.clone();
        let new_keywords = skill.trigger.keywords.clone();

        // Update skill
        {
            let mut skills = self.skills_by_id.write().await;
            skills.insert(skill_id, Arc::new(skill));
        }

        // Update name index
        {
            let mut names = self.skills_by_name.write().await;
            // Note: old name still maps to this ID, which is fine
            names.insert(new_name, skill_id);
        }

        // Add new keyword index entries
        {
            let mut index = self.keyword_index.write().await;
            for keyword in new_keywords {
                let keyword_lower = keyword.to_lowercase();
                let ids = index.entry(keyword_lower).or_default();
                if !ids.contains(&skill_id) {
                    ids.push(skill_id);
                }
            }
        }

        debug!("Updated skill: {}", skill_id);
        Ok(())
    }

    /// Clear all skills from the registry
    pub async fn clear(&self) {
        let mut skills = self.skills_by_id.write().await;
        let mut names = self.skills_by_name.write().await;
        let mut index = self.keyword_index.write().await;

        skills.clear();
        names.clear();
        index.clear();

        info!("Cleared skill registry");
    }

    /// Load skills from an iterator (useful for bulk loading from storage)
    pub async fn load_all(&self, skills: impl IntoIterator<Item = Skill>) -> Result<usize> {
        let mut count = 0;
        for skill in skills {
            self.register(skill).await?;
            count += 1;
        }
        info!("Loaded {} skills into registry", count);
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::SkillTrigger;

    fn create_test_skill(name: &str, keywords: Vec<&str>) -> Skill {
        Skill::new(name, format!("Test skill: {}", name), SkillCategory::Custom).with_trigger(
            SkillTrigger::with_keywords(keywords.into_iter().map(String::from).collect()),
        )
    }

    #[tokio::test]
    async fn test_register_and_get() {
        let registry = SkillRegistry::new();
        let skill = create_test_skill("test", vec!["hello", "world"]);
        let skill_id = skill.id;

        registry.register(skill.clone()).await.unwrap();

        let retrieved = registry.get(skill_id).await.unwrap();
        assert_eq!(retrieved.name, "test");
    }

    #[tokio::test]
    async fn test_get_by_name() {
        let registry = SkillRegistry::new();
        let skill = create_test_skill("my_skill", vec![]);

        registry.register(skill).await.unwrap();

        let retrieved = registry.get_by_name("my_skill").await.unwrap();
        assert_eq!(retrieved.name, "my_skill");

        assert!(registry.get_by_name("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_keyword_lookup() {
        let registry = SkillRegistry::new();

        let skill1 = create_test_skill("skill1", vec!["read", "file"]);
        let skill2 = create_test_skill("skill2", vec!["read", "database"]);
        let skill3 = create_test_skill("skill3", vec!["write", "file"]);

        registry.register(skill1).await.unwrap();
        registry.register(skill2).await.unwrap();
        registry.register(skill3).await.unwrap();

        let read_skills = registry.get_by_keyword("read").await;
        assert_eq!(read_skills.len(), 2);

        let file_skills = registry.get_by_keyword("file").await;
        assert_eq!(file_skills.len(), 2);

        let db_skills = registry.get_by_keyword("database").await;
        assert_eq!(db_skills.len(), 1);

        // Case insensitive
        let read_upper = registry.get_by_keyword("READ").await;
        assert_eq!(read_upper.len(), 2);
    }

    #[tokio::test]
    async fn test_unregister() {
        let registry = SkillRegistry::new();
        let skill = create_test_skill("test", vec!["keyword"]);
        let skill_id = skill.id;

        registry.register(skill).await.unwrap();
        assert!(registry.get(skill_id).await.is_some());

        registry.unregister(skill_id).await.unwrap();
        assert!(registry.get(skill_id).await.is_none());
        assert!(registry.get_by_name("test").await.is_none());
        assert!(registry.get_by_keyword("keyword").await.is_empty());
    }

    #[tokio::test]
    async fn test_update() {
        let registry = SkillRegistry::new();
        let mut skill = create_test_skill("test", vec!["old_keyword"]);
        let skill_id = skill.id;

        registry.register(skill.clone()).await.unwrap();

        // Update the skill
        skill.trigger.keywords = vec!["new_keyword".to_string()];
        registry.update(skill).await.unwrap();

        // Old keyword should not work
        let old_results = registry.get_by_keyword("old_keyword").await;
        assert!(old_results.is_empty());

        // New keyword should work
        let new_results = registry.get_by_keyword("new_keyword").await;
        assert_eq!(new_results.len(), 1);
        assert_eq!(new_results[0].id, skill_id);
    }

    #[tokio::test]
    async fn test_count() {
        let registry = SkillRegistry::new();

        assert_eq!(registry.count().await, 0);

        let mut skill1 = create_test_skill("skill1", vec![]);
        skill1.activate();
        registry.register(skill1).await.unwrap();

        let skill2 = create_test_skill("skill2", vec![]); // Draft status
        registry.register(skill2).await.unwrap();

        assert_eq!(registry.count().await, 2);
        assert_eq!(registry.count_active().await, 1);
    }

    #[tokio::test]
    async fn test_duplicate_name_error() {
        let registry = SkillRegistry::new();

        let skill1 = create_test_skill("same_name", vec![]);
        let skill2 = create_test_skill("same_name", vec![]);

        registry.register(skill1).await.unwrap();
        let result = registry.register(skill2).await;

        assert!(result.is_err());
    }
}
