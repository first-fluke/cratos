//! Skill Ecosystem - Export/Import and Sharing
//!
//! This module provides functionality for sharing skills between users:
//! - Export skills to JSON/YAML files
//! - Import skills from files or URLs
//! - Validate imported skills
//! - Version management
//!
//! ## Portable Skill Format
//!
//! Skills are exported in a portable format that includes:
//! - Skill definition (name, description, triggers, steps)
//! - Version information
//! - Author metadata
//! - Checksum for integrity verification
//!
//! ## Example
//!
//! ```ignore
//! use cratos_skills::{SkillEcosystem, SkillStore, ExportFormat};
//!
//! let store = SkillStore::from_path(&path).await?;
//! let ecosystem = SkillEcosystem::new(store);
//!
//! // Export a skill
//! let portable = ecosystem.export_skill(skill_id).await?;
//! portable.save_to_file("my_skill.yaml", ExportFormat::Yaml)?;
//!
//! // Import a skill
//! let imported = ecosystem.import_from_file("shared_skill.yaml").await?;
//! println!("Imported: {}", imported.name);
//!
//! // Export all skills as a bundle
//! let bundle = ecosystem.export_bundle("my_skills").await?;
//! bundle.save_to_file("skills_bundle.yaml")?;
//! ```

use crate::error::{Error, Result};
use crate::skill::{Skill, SkillCategory, SkillOrigin, SkillStatus, SkillStep, SkillTrigger};
use crate::store::SkillStore;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportFormat {
    /// JSON format (compact)
    Json,
    /// JSON format (pretty-printed)
    JsonPretty,
    /// YAML format (human-readable, default)
    #[default]
    Yaml,
}

impl ExportFormat {
    /// Get file extension for this format
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Json | Self::JsonPretty => "json",
            Self::Yaml => "yaml",
        }
    }

    /// Detect format from file extension
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }
}

/// Portable skill format for export/import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableSkill {
    /// Format version for compatibility
    pub format_version: String,

    /// Skill definition
    pub skill: Arc<PortableSkillDef>,

    /// Export metadata
    pub export_info: Arc<ExportInfo>,

    /// Checksum for integrity verification
    pub checksum: String,
}

/// Skill definition in portable format (without internal IDs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableSkillDef {
    /// Human-readable name
    pub name: String,

    /// Description of what the skill does
    pub description: String,

    /// Category
    pub category: String,

    /// Trigger configuration
    pub trigger: PortableTrigger,

    /// Execution steps
    pub steps: Vec<PortableStep>,

    /// JSON Schema for input validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Portable trigger format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableTrigger {
    /// Keywords that can trigger this skill
    #[serde(default)]
    pub keywords: Vec<String>,

    /// Regex patterns that can trigger this skill
    #[serde(default)]
    pub regex_patterns: Vec<String>,

    /// Intent classifications
    #[serde(default)]
    pub intents: Vec<String>,

    /// Priority (higher = more preferred)
    #[serde(default)]
    pub priority: i32,
}

/// Portable step format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableStep {
    /// Order of execution (1-based)
    pub order: u32,

    /// Tool name to invoke
    pub tool_name: String,

    /// Input template with variable placeholders
    pub input_template: serde_json::Value,

    /// Action to take on error
    #[serde(default = "default_on_error")]
    pub on_error: String,

    /// Description of what this step does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_on_error() -> String {
    "abort".to_string()
}

/// Export metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportInfo {
    /// When the skill was exported
    pub exported_at: DateTime<Utc>,

    /// Cratos version that exported the skill
    pub cratos_version: String,

    /// Author information (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Source URL or repository (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,

    /// License (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// Skill bundle for exporting multiple skills
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBundle {
    /// Format version
    pub format_version: String,

    /// Bundle name
    pub name: String,

    /// Bundle description
    pub description: String,

    /// Skills in the bundle
    pub skills: Vec<Arc<PortableSkillDef>>,

    /// Export metadata
    pub export_info: Arc<ExportInfo>,

    /// Bundle checksum
    pub checksum: String,
}

/// Import result with validation info
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Imported skill
    pub skill: Skill,

    /// Whether the skill was newly created or updated
    pub is_new: bool,

    /// Validation warnings (non-fatal issues)
    pub warnings: Vec<String>,
}

/// Skill ecosystem manager
#[derive(Clone)]
pub struct SkillEcosystem {
    store: SkillStore,
}

impl SkillEcosystem {
    /// Create a new skill ecosystem manager
    #[must_use]
    pub fn new(store: SkillStore) -> Self {
        Self { store }
    }

    /// Export a skill to portable format
    pub async fn export_skill(&self, skill_id: Uuid) -> Result<PortableSkill> {
        let skill = self.store.get_skill(skill_id).await?;
        Ok(self.skill_to_portable(&skill))
    }

    /// Export a skill by name
    pub async fn export_skill_by_name(&self, name: &str) -> Result<PortableSkill> {
        let skill = self
            .store
            .get_skill_by_name(name)
            .await?
            .ok_or_else(|| Error::NotFound(format!("skill not found: {name}")))?;

        Ok(self.skill_to_portable(&skill))
    }

    /// Export multiple skills as a bundle
    pub async fn export_bundle(&self, name: &str, description: &str) -> Result<SkillBundle> {
        let skills = self.store.list_active_skills().await?;
        self.create_bundle(name, description, &skills)
    }

    /// Export selected skills as a bundle
    pub async fn export_skills_as_bundle(
        &self,
        name: &str,
        description: &str,
        skill_ids: &[Uuid],
    ) -> Result<SkillBundle> {
        let mut skills = Vec::new();
        for id in skill_ids {
            match self.store.get_skill(*id).await {
                Ok(skill) => skills.push(skill),
                Err(_) => continue, // Skip skills that don't exist
            }
        }
        self.create_bundle(name, description, &skills)
    }

    /// Import a skill from portable format
    pub async fn import_skill(&self, portable: &PortableSkill) -> Result<ImportResult> {
        // Validate checksum
        let expected_checksum = Self::calculate_checksum(&portable.skill);
        if portable.checksum != expected_checksum {
            warn!(
                "Checksum mismatch for skill '{}': expected {}, got {}",
                portable.skill.name, expected_checksum, portable.checksum
            );
            // Continue with warning, don't fail
        }

        // Validate format version
        let warnings = self.validate_portable(&portable.skill);

        // Check if skill already exists
        let existing = self.store.get_skill_by_name(&portable.skill.name).await?;

        let (skill, is_new) = if let Some(mut existing) = existing {
            // Update existing skill
            self.update_skill_from_portable(&mut existing, &portable.skill);
            self.store.save_skill(&existing).await?;
            info!("Updated existing skill: {}", existing.name);
            (existing, false)
        } else {
            // Create new skill
            let skill = self.portable_to_skill(&portable.skill);
            self.store.save_skill(&skill).await?;
            info!("Imported new skill: {}", skill.name);
            (skill, true)
        };

        Ok(ImportResult {
            skill,
            is_new,
            warnings,
        })
    }

    /// Import a skill from a file
    pub async fn import_from_file(&self, path: &Path) -> Result<ImportResult> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Internal(format!("failed to read file: {e}")))?;

        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(ExportFormat::from_extension)
            .unwrap_or(ExportFormat::Yaml);

        let portable: PortableSkill = match format {
            ExportFormat::Json | ExportFormat::JsonPretty => serde_json::from_str(&content)
                .map_err(|e| Error::Parse(format!("invalid JSON: {e}")))?,
            ExportFormat::Yaml => serde_yaml::from_str(&content)
                .map_err(|e| Error::Parse(format!("invalid YAML: {e}")))?,
        };

        self.import_skill(&portable).await
    }

    /// Import a bundle of skills
    pub async fn import_bundle(&self, bundle: &SkillBundle) -> Result<Vec<ImportResult>> {
        let mut results = Vec::new();

        for skill_def in &bundle.skills {
            let portable = PortableSkill {
                format_version: bundle.format_version.clone(),
                skill: Arc::clone(skill_def),
                export_info: Arc::clone(&bundle.export_info),
                checksum: Self::calculate_checksum(skill_def),
            };

            match self.import_skill(&portable).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!("Failed to import skill '{}': {}", skill_def.name, e);
                }
            }
        }

        info!(
            "Imported {} skills from bundle '{}'",
            results.len(),
            bundle.name
        );
        Ok(results)
    }

    /// Import a bundle from a file
    pub async fn import_bundle_from_file(&self, path: &Path) -> Result<Vec<ImportResult>> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Internal(format!("failed to read file: {e}")))?;

        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(ExportFormat::from_extension)
            .unwrap_or(ExportFormat::Yaml);

        let bundle: SkillBundle = match format {
            ExportFormat::Json | ExportFormat::JsonPretty => serde_json::from_str(&content)
                .map_err(|e| Error::Parse(format!("invalid JSON: {e}")))?,
            ExportFormat::Yaml => serde_yaml::from_str(&content)
                .map_err(|e| Error::Parse(format!("invalid YAML: {e}")))?,
        };

        self.import_bundle(&bundle).await
    }

    // ========================================================================
    // Private helper methods
    // ========================================================================

    fn skill_to_portable(&self, skill: &Skill) -> PortableSkill {
        let skill_def = PortableSkillDef {
            name: skill.name.clone(),
            description: skill.description.clone(),
            category: skill.category.as_str().to_string(),
            trigger: PortableTrigger {
                keywords: skill.trigger.keywords.clone(),
                regex_patterns: skill.trigger.regex_patterns.clone(),
                intents: skill.trigger.intents.clone(),
                priority: skill.trigger.priority,
            },
            steps: skill
                .steps
                .iter()
                .map(|s| PortableStep {
                    order: s.order,
                    tool_name: s.tool_name.clone(),
                    input_template: s.input_template.clone(),
                    on_error: s.on_error.as_str().to_string(),
                    description: s.description.clone(),
                })
                .collect(),
            input_schema: skill.input_schema.clone(),
            tags: Vec::new(),
        };

        let checksum = Self::calculate_checksum(&skill_def);

        PortableSkill {
            format_version: "1.0".to_string(),
            skill: Arc::new(skill_def),
            export_info: Arc::new(ExportInfo {
                exported_at: Utc::now(),
                cratos_version: env!("CARGO_PKG_VERSION").to_string(),
                author: None,
                source_url: None,
                license: Some("MIT".to_string()),
            }),
            checksum,
        }
    }

    fn portable_to_skill(&self, def: &PortableSkillDef) -> Skill {
        let category = def.category.parse().unwrap_or(SkillCategory::Custom);

        let mut skill = Skill::new(&def.name, &def.description, category);
        skill.origin = SkillOrigin::UserDefined;
        skill.status = SkillStatus::Active;
        skill.trigger = SkillTrigger {
            keywords: def.trigger.keywords.clone(),
            regex_patterns: def.trigger.regex_patterns.clone(),
            intents: def.trigger.intents.clone(),
            priority: def.trigger.priority,
        };
        skill.steps = def
            .steps
            .iter()
            .map(|s| {
                let on_error = s.on_error.parse().unwrap_or_default();
                SkillStep {
                    order: s.order,
                    tool_name: s.tool_name.clone(),
                    input_template: s.input_template.clone(),
                    on_error,
                    description: s.description.clone(),
                    max_retries: 0,
                }
            })
            .collect();
        skill.input_schema = def.input_schema.clone();

        skill
    }

    fn update_skill_from_portable(&self, skill: &mut Skill, def: &PortableSkillDef) {
        skill.description = def.description.clone();
        skill.trigger = SkillTrigger {
            keywords: def.trigger.keywords.clone(),
            regex_patterns: def.trigger.regex_patterns.clone(),
            intents: def.trigger.intents.clone(),
            priority: def.trigger.priority,
        };
        skill.steps = def
            .steps
            .iter()
            .map(|s| {
                let on_error = s.on_error.parse().unwrap_or_default();
                SkillStep {
                    order: s.order,
                    tool_name: s.tool_name.clone(),
                    input_template: s.input_template.clone(),
                    on_error,
                    description: s.description.clone(),
                    max_retries: 0,
                }
            })
            .collect();
        skill.input_schema = def.input_schema.clone();
        skill.updated_at = Utc::now();
    }

    fn create_bundle(
        &self,
        name: &str,
        description: &str,
        skills: &[Skill],
    ) -> Result<SkillBundle> {
        let mut skill_defs = Vec::with_capacity(skills.len());
        for s in skills {
            let portable = self.skill_to_portable(s);
            skill_defs.push(portable.skill);
        }

        let bundle_content =
            serde_json::to_string(&skill_defs).map_err(|e| Error::Serialization(e.to_string()))?;
        let checksum = Self::calculate_hash(&bundle_content);

        Ok(SkillBundle {
            format_version: "1.0".to_string(),
            name: name.to_string(),
            description: description.to_string(),
            skills: skill_defs,
            export_info: Arc::new(ExportInfo {
                exported_at: Utc::now(),
                cratos_version: env!("CARGO_PKG_VERSION").to_string(),
                author: None,
                source_url: None,
                license: Some("MIT".to_string()),
            }),
            checksum,
        })
    }

    fn calculate_checksum(skill: &PortableSkillDef) -> String {
        let content = serde_json::to_string(skill).unwrap_or_default();
        Self::calculate_hash(&content)
    }

    fn calculate_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)[..16].to_string()
    }

    fn validate_portable(&self, def: &PortableSkillDef) -> Vec<String> {
        let mut warnings = Vec::new();

        if def.name.is_empty() {
            warnings.push("Skill name is empty".to_string());
        }

        if def.steps.is_empty() {
            warnings.push("Skill has no execution steps".to_string());
        }

        if def.trigger.keywords.is_empty()
            && def.trigger.regex_patterns.is_empty()
            && def.trigger.intents.is_empty()
        {
            warnings.push("Skill has no triggers defined".to_string());
        }

        // Check for potentially dangerous tool names
        for step in &def.steps {
            if step.tool_name.contains("..") || step.tool_name.contains('/') {
                warnings.push(format!(
                    "Step {} has suspicious tool name: {}",
                    step.order, step.tool_name
                ));
            }
        }

        warnings
    }
}

impl PortableSkill {
    /// Save to a file
    pub fn save_to_file(&self, path: &Path, format: ExportFormat) -> Result<()> {
        let content = match format {
            ExportFormat::Json => {
                serde_json::to_string(self).map_err(|e| Error::Serialization(e.to_string()))?
            }
            ExportFormat::JsonPretty => serde_json::to_string_pretty(self)
                .map_err(|e| Error::Serialization(e.to_string()))?,
            ExportFormat::Yaml => {
                serde_yaml::to_string(self).map_err(|e| Error::Serialization(e.to_string()))?
            }
        };

        std::fs::write(path, content)
            .map_err(|e| Error::Internal(format!("failed to write file: {e}")))?;

        info!("Exported skill '{}' to {:?}", self.skill.name, path);
        Ok(())
    }

    /// Load from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Internal(format!("failed to read file: {e}")))?;

        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(ExportFormat::from_extension)
            .unwrap_or(ExportFormat::Yaml);

        match format {
            ExportFormat::Json | ExportFormat::JsonPretty => serde_json::from_str(&content)
                .map_err(|e| Error::Parse(format!("invalid JSON: {e}"))),
            ExportFormat::Yaml => serde_yaml::from_str(&content)
                .map_err(|e| Error::Parse(format!("invalid YAML: {e}"))),
        }
    }
}

impl SkillBundle {
    /// Save to a file
    pub fn save_to_file(&self, path: &Path, format: ExportFormat) -> Result<()> {
        let content = match format {
            ExportFormat::Json => {
                serde_json::to_string(self).map_err(|e| Error::Serialization(e.to_string()))?
            }
            ExportFormat::JsonPretty => serde_json::to_string_pretty(self)
                .map_err(|e| Error::Serialization(e.to_string()))?,
            ExportFormat::Yaml => {
                serde_yaml::to_string(self).map_err(|e| Error::Serialization(e.to_string()))?
            }
        };

        std::fs::write(path, content)
            .map_err(|e| Error::Internal(format!("failed to write file: {e}")))?;

        info!(
            "Exported bundle '{}' ({} skills) to {:?}",
            self.name,
            self.skills.len(),
            path
        );
        Ok(())
    }

    /// Load from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Internal(format!("failed to read file: {e}")))?;

        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(ExportFormat::from_extension)
            .unwrap_or(ExportFormat::Yaml);

        match format {
            ExportFormat::Json | ExportFormat::JsonPretty => serde_json::from_str(&content)
                .map_err(|e| Error::Parse(format!("invalid JSON: {e}"))),
            ExportFormat::Yaml => serde_yaml::from_str(&content)
                .map_err(|e| Error::Parse(format!("invalid YAML: {e}"))),
        }
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests;
