//! Chronicles Store
//!
//! Stores chronicles as JSON files in `~/.cratos/chronicles/` directory

use super::Chronicle;
use crate::error::Result;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Default Chronicles directory
const DEFAULT_CHRONICLES_DIR: &str = ".cratos/chronicles";

/// Chronicles Store
#[derive(Debug)]
pub struct ChronicleStore {
    data_dir: PathBuf,
}

impl ChronicleStore {
    /// Create store with default path (`~/.cratos/chronicles/`)
    #[must_use]
    pub fn new() -> Self {
        let data_dir = dirs::home_dir()
            .map(|h| h.join(DEFAULT_CHRONICLES_DIR))
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CHRONICLES_DIR));

        Self { data_dir }
    }

    /// Create store with custom path
    #[must_use]
    pub fn with_path(path: impl AsRef<Path>) -> Self {
        Self {
            data_dir: path.as_ref().to_path_buf(),
        }
    }

    /// Return data directory path
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Save chronicle
    ///
    /// Filename format: `{persona_name}_lv{level}.json`
    pub fn save(&self, chronicle: &Chronicle) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.data_dir).map_err(crate::error::ChronicleError::from)?;

        let filename = Self::filename(&chronicle.persona_name, chronicle.level);
        let path = self.data_dir.join(&filename);

        let content = serde_json::to_string_pretty(chronicle)
            .map_err(crate::error::ChronicleError::from)?;

        std::fs::write(&path, content).map_err(crate::error::ChronicleError::from)?;

        info!(
            persona = %chronicle.persona_name,
            level = chronicle.level,
            path = ?path,
            "Chronicle saved"
        );
        Ok(path)
    }

    /// Load latest chronicle by persona name
    ///
    /// Returns chronicle with highest level
    pub fn load(&self, persona_name: &str) -> Result<Option<Chronicle>> {
        if !self.data_dir.exists() {
            return Ok(None);
        }

        let name_lower = persona_name.to_lowercase();
        let mut latest: Option<Chronicle> = None;

        let entries = std::fs::read_dir(&self.data_dir).map_err(crate::error::ChronicleError::from)?;

        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();

            if !filename.starts_with(&name_lower) || !filename.ends_with(".json") {
                continue;
            }

            match self.load_file(&entry.path()) {
                Ok(chronicle) => {
                    if latest.as_ref().is_none_or(|l| chronicle.level > l.level) {
                        latest = Some(chronicle);
                    }
                }
                Err(e) => {
                    warn!(path = ?entry.path(), error = %e, "Failed to load chronicle");
                }
            }
        }

        if let Some(ref c) = latest {
            debug!(persona = %c.persona_name, level = c.level, "Chronicle loaded");
        }

        Ok(latest)
    }

    /// Load chronicle at specific level
    pub fn load_level(&self, persona_name: &str, level: u8) -> Result<Option<Chronicle>> {
        let filename = Self::filename(persona_name, level);
        let path = self.data_dir.join(&filename);

        if !path.exists() {
            return Ok(None);
        }

        self.load_file(&path).map(Some)
    }

    /// Load all chronicles (latest version for each persona)
    pub fn load_all(&self) -> Result<Vec<Chronicle>> {
        let mut chronicles = Vec::new();
        let mut seen_personas = std::collections::HashSet::new();

        if !self.data_dir.exists() {
            return Ok(chronicles);
        }

        let entries = std::fs::read_dir(&self.data_dir).map_err(crate::error::ChronicleError::from)?;

        // Sort by filename (reversed for descending level order)
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort_by(|a, b| b.cmp(a));

        for path in paths {
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }

            match self.load_file(&path) {
                Ok(chronicle) => {
                    let name_lower = chronicle.persona_name.to_lowercase();
                    if !seen_personas.contains(&name_lower) {
                        seen_personas.insert(name_lower);
                        chronicles.push(chronicle);
                    }
                }
                Err(e) => {
                    warn!(path = ?path, error = %e, "Failed to load chronicle");
                }
            }
        }

        Ok(chronicles)
    }

    /// Return list of available persona names
    pub fn list_personas(&self) -> Result<Vec<String>> {
        let mut personas = std::collections::HashSet::new();

        if !self.data_dir.exists() {
            return Ok(Vec::new());
        }

        let entries = std::fs::read_dir(&self.data_dir).map_err(crate::error::ChronicleError::from)?;

        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.ends_with(".json") {
                // Extract persona name from filename (persona_lvN.json)
                if let Some(name) = filename.split("_lv").next() {
                    personas.insert(name.to_string());
                }
            }
        }

        let mut result: Vec<_> = personas.into_iter().collect();
        result.sort();
        Ok(result)
    }

    /// Delete chronicle
    pub fn delete(&self, persona_name: &str, level: u8) -> Result<bool> {
        let filename = Self::filename(persona_name, level);
        let path = self.data_dir.join(&filename);

        if !path.exists() {
            return Ok(false);
        }

        std::fs::remove_file(&path).map_err(crate::error::ChronicleError::from)?;

        info!(persona = %persona_name, level, "Chronicle deleted");
        Ok(true)
    }

    /// Check if chronicle exists
    #[must_use]
    pub fn exists(&self, persona_name: &str) -> bool {
        if !self.data_dir.exists() {
            return false;
        }

        let name_lower = persona_name.to_lowercase();

        std::fs::read_dir(&self.data_dir)
            .map(|entries| {
                entries.flatten().any(|e| {
                    let filename = e.file_name().to_string_lossy().to_string();
                    filename.starts_with(&name_lower) && filename.ends_with(".json")
                })
            })
            .unwrap_or(false)
    }

    fn load_file(&self, path: &Path) -> Result<Chronicle> {
        let content = std::fs::read_to_string(path)
            .map_err(crate::error::ChronicleError::from)?;

        serde_json::from_str(&content).map_err(|e| crate::error::ChronicleError::from(e).into())
    }

    fn filename(persona_name: &str, level: u8) -> String {
        format!("{}_lv{}.json", persona_name.to_lowercase(), level)
    }
}

impl Default for ChronicleStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;

