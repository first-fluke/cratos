//! Decree TOML Loader

use super::{Laws, Ranks, Warfare};
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Default Decrees directory
const DEFAULT_DECREES_DIR: &str = "config/decrees";

/// Decree Loader
#[derive(Debug)]
pub struct DecreeLoader {
    config_dir: PathBuf,
}

impl DecreeLoader {
    /// Create loader with default path (`config/decrees/`)
    #[must_use]
    pub fn new() -> Self {
        Self {
            config_dir: PathBuf::from(DEFAULT_DECREES_DIR),
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

    /// Load laws
    pub fn load_laws(&self) -> Result<Laws> {
        let path = self.config_dir.join("laws.toml");
        self.load_file(&path)
    }

    /// Load rank system
    pub fn load_ranks(&self) -> Result<Ranks> {
        let path = self.config_dir.join("ranks.toml");
        self.load_file(&path)
    }

    /// Load development rules
    pub fn load_warfare(&self) -> Result<Warfare> {
        let path = self.config_dir.join("warfare.toml");
        self.load_file(&path)
    }

    /// Load alliance (collaboration rules)
    pub fn load_alliance(&self) -> Result<Laws> {
        let path = self.config_dir.join("alliance.toml");
        self.load_file(&path)
    }

    /// Load tribute (cost/budget rules)
    pub fn load_tribute(&self) -> Result<Laws> {
        let path = self.config_dir.join("tribute.toml");
        self.load_file(&path)
    }

    /// Load judgment (evaluation framework)
    pub fn load_judgment(&self) -> Result<Laws> {
        let path = self.config_dir.join("judgment.toml");
        self.load_file(&path)
    }

    /// Load culture (values and communication)
    pub fn load_culture(&self) -> Result<Laws> {
        let path = self.config_dir.join("culture.toml");
        self.load_file(&path)
    }

    /// Load operations (operational procedures)
    pub fn load_operations(&self) -> Result<Laws> {
        let path = self.config_dir.join("operations.toml");
        self.load_file(&path)
    }

    /// Check if laws exist
    #[must_use]
    pub fn laws_exists(&self) -> bool {
        self.config_dir.join("laws.toml").exists()
    }

    /// Check if rank system exists
    #[must_use]
    pub fn ranks_exists(&self) -> bool {
        self.config_dir.join("ranks.toml").exists()
    }

    /// Check if development rules exist
    #[must_use]
    pub fn warfare_exists(&self) -> bool {
        self.config_dir.join("warfare.toml").exists()
    }

    /// Check if alliance rules exist
    #[must_use]
    pub fn alliance_exists(&self) -> bool {
        self.config_dir.join("alliance.toml").exists()
    }

    /// Check if tribute rules exist
    #[must_use]
    pub fn tribute_exists(&self) -> bool {
        self.config_dir.join("tribute.toml").exists()
    }

    /// Check if judgment framework exists
    #[must_use]
    pub fn judgment_exists(&self) -> bool {
        self.config_dir.join("judgment.toml").exists()
    }

    /// Check if culture rules exist
    #[must_use]
    pub fn culture_exists(&self) -> bool {
        self.config_dir.join("culture.toml").exists()
    }

    /// Check if operations rules exist
    #[must_use]
    pub fn operations_exists(&self) -> bool {
        self.config_dir.join("operations.toml").exists()
    }

    /// Validate all decrees
    pub fn validate_all(&self) -> ValidationResult {
        let mut result = ValidationResult::default();

        // Check Laws
        if self.laws_exists() {
            match self.load_laws() {
                Ok(laws) => {
                    result.laws_count = Some(laws.article_count());
                    result.laws_valid = laws.is_valid();
                }
                Err(e) => {
                    warn!(error = %e, "Failed to load laws");
                    result.laws_error = Some(e.to_string());
                }
            }
        }

        // Check Ranks
        if self.ranks_exists() {
            match self.load_ranks() {
                Ok(ranks) => {
                    result.ranks_count = Some(ranks.rank_count());
                    result.ranks_valid = ranks.is_valid();
                }
                Err(e) => {
                    warn!(error = %e, "Failed to load ranks");
                    result.ranks_error = Some(e.to_string());
                }
            }
        }

        // Check Warfare
        if self.warfare_exists() {
            match self.load_warfare() {
                Ok(warfare) => {
                    result.warfare_count = Some(warfare.section_count());
                    result.warfare_valid = warfare.is_valid();
                }
                Err(e) => {
                    warn!(error = %e, "Failed to load warfare");
                    result.warfare_error = Some(e.to_string());
                }
            }
        }

        // Check extended decrees
        let extended_names = ["alliance", "tribute", "judgment", "culture", "operations"];
        for name in extended_names {
            let exists = match name {
                "alliance" => self.alliance_exists(),
                "tribute" => self.tribute_exists(),
                "judgment" => self.judgment_exists(),
                "culture" => self.culture_exists(),
                "operations" => self.operations_exists(),
                _ => false,
            };
            if !exists {
                continue;
            }
            let load_result = match name {
                "alliance" => self.load_alliance(),
                "tribute" => self.load_tribute(),
                "judgment" => self.load_judgment(),
                "culture" => self.load_culture(),
                "operations" => self.load_operations(),
                _ => continue,
            };
            match load_result {
                Ok(decree) => {
                    result.extended.push(ExtendedDecreeResult {
                        name: name.to_string(),
                        count: Some(decree.article_count()),
                        valid: decree.article_count() > 0,
                        error: None,
                    });
                }
                Err(e) => {
                    warn!(error = %e, decree = name, "Failed to load extended decree");
                    result.extended.push(ExtendedDecreeResult {
                        name: name.to_string(),
                        count: None,
                        valid: false,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        result
    }

    fn load_file<T>(&self, path: &Path) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        if !path.exists() {
            return Err(Error::Internal(format!(
                "Decree file not found: {}",
                path.display()
            )));
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Internal(format!("Failed to read {}: {}", path.display(), e)))?;

        let decree: T = toml::from_str(&content)
            .map_err(|e| Error::Internal(format!("Failed to parse {}: {}", path.display(), e)))?;

        debug!(path = ?path, "Decree loaded");
        Ok(decree)
    }
}

impl Default for DecreeLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Result for an extended decree validation
#[derive(Debug)]
pub struct ExtendedDecreeResult {
    /// Decree name (e.g., "alliance", "tribute")
    pub name: String,
    /// Article count
    pub count: Option<usize>,
    /// Valid flag
    pub valid: bool,
    /// Error message
    pub error: Option<String>,
}

/// Validation result
#[derive(Debug, Default)]
pub struct ValidationResult {
    /// Laws article count
    pub laws_count: Option<usize>,
    /// Laws valid flag
    pub laws_valid: bool,
    /// Laws error message
    pub laws_error: Option<String>,

    /// Rank count
    pub ranks_count: Option<usize>,
    /// Ranks valid flag
    pub ranks_valid: bool,
    /// Ranks error message
    pub ranks_error: Option<String>,

    /// Section count
    pub warfare_count: Option<usize>,
    /// Warfare valid flag
    pub warfare_valid: bool,
    /// Warfare error message
    pub warfare_error: Option<String>,

    /// Extended decree results
    pub extended: Vec<ExtendedDecreeResult>,
}

impl ValidationResult {
    /// Check if all decrees are valid
    #[must_use]
    pub fn all_valid(&self) -> bool {
        self.laws_valid && self.ranks_valid && self.warfare_valid
    }

    /// Check if required decrees (laws) are valid
    #[must_use]
    pub fn required_valid(&self) -> bool {
        self.laws_valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_laws_toml() -> &'static str {
        r#"
[meta]
title = "Laws (LAWS)"
philosophy = "Test philosophy"
immutable = true

[[articles]]
id = 1
title = "Article 1"
rules = ["Rule 1", "Rule 2"]

[[articles]]
id = 2
title = "Article 2"
rules = ["Rule A"]
"#
    }

    #[test]
    fn test_loader_new() {
        let loader = DecreeLoader::new();
        assert!(loader.config_dir().ends_with("decrees"));
    }

    #[test]
    fn test_loader_with_path() {
        let loader = DecreeLoader::with_path("/custom/path");
        assert_eq!(loader.config_dir(), Path::new("/custom/path"));
    }

    #[test]
    fn test_load_laws() {
        let temp_dir = TempDir::new().unwrap();
        let loader = DecreeLoader::with_path(temp_dir.path());

        // When file doesn't exist
        assert!(loader.load_laws().is_err());

        // Create file
        fs::write(temp_dir.path().join("laws.toml"), create_test_laws_toml()).unwrap();

        let laws = loader.load_laws().unwrap();
        assert_eq!(laws.article_count(), 2);
        assert_eq!(laws.meta.title, "Laws (LAWS)");
    }

    #[test]
    fn test_exists_methods() {
        let temp_dir = TempDir::new().unwrap();
        let loader = DecreeLoader::with_path(temp_dir.path());

        assert!(!loader.laws_exists());
        assert!(!loader.ranks_exists());
        assert!(!loader.warfare_exists());

        fs::write(temp_dir.path().join("laws.toml"), "").unwrap();
        assert!(loader.laws_exists());
    }

    #[test]
    fn test_validate_all() {
        let temp_dir = TempDir::new().unwrap();
        let loader = DecreeLoader::with_path(temp_dir.path());

        // When file doesn't exist
        let result = loader.validate_all();
        assert!(!result.all_valid());
        assert!(result.laws_count.is_none());

        // Create file
        fs::write(temp_dir.path().join("laws.toml"), create_test_laws_toml()).unwrap();

        let result = loader.validate_all();
        assert_eq!(result.laws_count, Some(2));
        assert!(!result.laws_valid); // less than 10
    }

    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::default();

        assert!(!result.all_valid());
        assert!(!result.required_valid());

        result.laws_valid = true;
        assert!(result.required_valid());
        assert!(!result.all_valid());

        result.ranks_valid = true;
        result.warfare_valid = true;
        assert!(result.all_valid());
    }
}
