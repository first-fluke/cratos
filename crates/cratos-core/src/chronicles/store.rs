//! Chronicles Store
//!
//! Stores chronicles as JSON files in `~/.cratos/chronicles/` directory

use super::Chronicle;
use crate::error::{Error, Result};
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
        std::fs::create_dir_all(&self.data_dir).map_err(|e| {
            Error::Internal(format!(
                "Failed to create chronicles directory {:?}: {}",
                self.data_dir, e
            ))
        })?;

        let filename = Self::filename(&chronicle.persona_name, chronicle.level);
        let path = self.data_dir.join(&filename);

        let content = serde_json::to_string_pretty(chronicle)
            .map_err(|e| Error::Internal(format!("Failed to serialize chronicle: {}", e)))?;

        std::fs::write(&path, content).map_err(|e| {
            Error::Internal(format!("Failed to write chronicle to {:?}: {}", path, e))
        })?;

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

        let entries = std::fs::read_dir(&self.data_dir).map_err(|e| {
            Error::Internal(format!(
                "Failed to read chronicles directory {:?}: {}",
                self.data_dir, e
            ))
        })?;

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

        let entries = std::fs::read_dir(&self.data_dir).map_err(|e| {
            Error::Internal(format!(
                "Failed to read chronicles directory {:?}: {}",
                self.data_dir, e
            ))
        })?;

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

        let entries = std::fs::read_dir(&self.data_dir).map_err(|e| {
            Error::Internal(format!(
                "Failed to read chronicles directory {:?}: {}",
                self.data_dir, e
            ))
        })?;

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

        std::fs::remove_file(&path).map_err(|e| {
            Error::Internal(format!("Failed to delete chronicle {:?}: {}", path, e))
        })?;

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
            .map_err(|e| Error::Internal(format!("Failed to read {:?}: {}", path, e)))?;

        serde_json::from_str(&content)
            .map_err(|e| Error::Internal(format!("Failed to parse {:?}: {}", path, e)))
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
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_chronicle(name: &str, level: u8) -> Chronicle {
        let mut chronicle = Chronicle::new(name);
        chronicle.level = level;
        chronicle.add_entry("Test task", Some("1"));
        chronicle
    }

    #[test]
    fn test_store_new() {
        let store = ChronicleStore::new();
        assert!(store.data_dir().ends_with("chronicles"));
    }

    #[test]
    fn test_store_with_path() {
        let store = ChronicleStore::with_path("/custom/path");
        assert_eq!(store.data_dir(), Path::new("/custom/path"));
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        let chronicle = create_test_chronicle("sindri", 1);
        let path = store.save(&chronicle).unwrap();

        assert!(path.exists());
        assert!(path.to_string_lossy().contains("sindri_lv1.json"));

        let loaded = store.load("sindri").unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.persona_name, "sindri");
        assert_eq!(loaded.level, 1);
    }

    #[test]
    fn test_load_latest_level() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        // Save multiple levels
        store.save(&create_test_chronicle("athena", 1)).unwrap();
        store.save(&create_test_chronicle("athena", 2)).unwrap();
        store.save(&create_test_chronicle("athena", 3)).unwrap();

        let loaded = store.load("athena").unwrap().unwrap();
        assert_eq!(loaded.level, 3); // Latest level
    }

    #[test]
    fn test_load_specific_level() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        store.save(&create_test_chronicle("heimdall", 1)).unwrap();
        store.save(&create_test_chronicle("heimdall", 2)).unwrap();

        let loaded = store.load_level("heimdall", 1).unwrap().unwrap();
        assert_eq!(loaded.level, 1);
    }

    #[test]
    fn test_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        let loaded = store.load("nonexistent").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_load_all() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        store.save(&create_test_chronicle("sindri", 1)).unwrap();
        store.save(&create_test_chronicle("athena", 2)).unwrap();
        store.save(&create_test_chronicle("heimdall", 1)).unwrap();

        let all = store.load_all().unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_list_personas() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        store.save(&create_test_chronicle("sindri", 1)).unwrap();
        store.save(&create_test_chronicle("athena", 1)).unwrap();
        store.save(&create_test_chronicle("sindri", 2)).unwrap(); // duplicate

        let personas = store.list_personas().unwrap();
        assert_eq!(personas.len(), 2);
        assert!(personas.contains(&"sindri".to_string()));
        assert!(personas.contains(&"athena".to_string()));
    }

    #[test]
    fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        store.save(&create_test_chronicle("mimir", 1)).unwrap();
        assert!(store.exists("mimir"));

        let deleted = store.delete("mimir", 1).unwrap();
        assert!(deleted);
        assert!(!store.exists("mimir"));
    }

    #[test]
    fn test_exists() {
        let temp_dir = TempDir::new().unwrap();
        let store = ChronicleStore::with_path(temp_dir.path());

        assert!(!store.exists("sindri"));

        store.save(&create_test_chronicle("sindri", 1)).unwrap();
        assert!(store.exists("sindri"));
        assert!(store.exists("SINDRI")); // case-insensitive
    }

    #[test]
    fn test_filename() {
        assert_eq!(ChronicleStore::filename("Sindri", 1), "sindri_lv1.json");
        assert_eq!(ChronicleStore::filename("ATHENA", 3), "athena_lv3.json");
    }
}
