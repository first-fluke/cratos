//! Persona TOML Loader
//!
//! Loads TOML files from the `config/pantheon/` directory.

use super::PersonaPreset;
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Default persona configuration directory
const DEFAULT_PANTHEON_DIR: &str = "config/pantheon";

/// Persona TOML Loader
#[derive(Debug)]
pub struct PersonaLoader {
    config_dir: PathBuf,
}

impl PersonaLoader {
    /// Create loader with default path (`config/pantheon/`)
    #[must_use]
    pub fn new() -> Self {
        Self {
            config_dir: PathBuf::from(DEFAULT_PANTHEON_DIR),
        }
    }

    /// Create loader with custom path
    #[must_use]
    pub fn with_path(path: impl AsRef<Path>) -> Self {
        Self {
            config_dir: path.as_ref().to_path_buf(),
        }
    }

    /// Return configuration directory path
    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Load all personas
    ///
    /// # Errors
    /// - Directory read failure
    /// - Individual file parse failures are warned and skipped
    pub fn load_all(&self) -> Result<Vec<PersonaPreset>> {
        let mut presets = Vec::new();

        if !self.config_dir.exists() {
            warn!("Pantheon directory not found: {:?}", self.config_dir);
            return Ok(presets);
        }

        let entries = std::fs::read_dir(&self.config_dir).map_err(|e| {
            Error::Configuration(format!(
                "Failed to read pantheon directory {:?}: {}",
                self.config_dir, e
            ))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();

            if !Self::is_toml_file(&path) {
                continue;
            }

            match self.load_file(&path) {
                Ok(preset) => {
                    info!(
                        "Loaded persona: {} ({})",
                        preset.persona.name, preset.persona.domain
                    );
                    presets.push(preset);
                }
                Err(e) => {
                    warn!("Failed to load {:?}: {}", path, e);
                }
            }
        }

        // Sort by priority (level) descending (Supreme first)
        presets.sort_by(|a, b| b.level.level.cmp(&a.level.level));

        debug!(
            "Loaded {} personas from {:?}",
            presets.len(),
            self.config_dir
        );
        Ok(presets)
    }

    /// Load single persona by name
    ///
    /// # Arguments
    /// * `name` - Persona name (case-insensitive)
    ///
    /// # Errors
    /// - File read failure
    /// - TOML parse failure
    pub fn load(&self, name: &str) -> Result<PersonaPreset> {
        let filename = format!("{}.toml", name.to_lowercase());
        let path = self.config_dir.join(&filename);

        if !path.exists() {
            return Err(Error::Configuration(format!(
                "Persona not found: {} (expected at {:?})",
                name, path
            )));
        }

        self.load_file(&path)
    }

    /// Check if persona exists
    #[must_use]
    pub fn exists(&self, name: &str) -> bool {
        let filename = format!("{}.toml", name.to_lowercase());
        self.config_dir.join(filename).exists()
    }

    /// Return list of available persona names
    pub fn list_names(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();

        if !self.config_dir.exists() {
            return Ok(names);
        }

        let entries = std::fs::read_dir(&self.config_dir).map_err(|e| {
            Error::Configuration(format!(
                "Failed to read pantheon directory {:?}: {}",
                self.config_dir, e
            ))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if Self::is_toml_file(&path) {
                if let Some(stem) = path.file_stem() {
                    names.push(stem.to_string_lossy().to_string());
                }
            }
        }

        names.sort();
        Ok(names)
    }

    fn load_file(&self, path: &Path) -> Result<PersonaPreset> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Configuration(format!("Failed to read {:?}: {}", path, e)))?;

        toml::from_str(&content)
            .map_err(|e| Error::Configuration(format!("Failed to parse {:?}: {}", path, e)))
    }

    fn is_toml_file(path: &Path) -> bool {
        path.extension().is_some_and(|ext| ext == "toml")
    }
}

impl Default for PersonaLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests;
