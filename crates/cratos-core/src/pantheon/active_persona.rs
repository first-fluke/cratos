//! Active Persona State
//!
//! Persists the currently summoned persona to a file at `~/.cratos/active_persona`.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Default state file path
const DEFAULT_STATE_FILE: &str = ".cratos/active_persona";

/// Manages the currently active (summoned) persona
#[derive(Debug)]
pub struct ActivePersonaState {
    state_file: PathBuf,
}

impl ActivePersonaState {
    /// Create with default path (`~/.cratos/active_persona`)
    #[must_use]
    pub fn new() -> Self {
        let state_file = dirs::home_dir()
            .map(|h| h.join(DEFAULT_STATE_FILE))
            .unwrap_or_else(|| PathBuf::from(DEFAULT_STATE_FILE));

        Self { state_file }
    }

    /// Create with custom path
    #[must_use]
    pub fn with_path(path: impl AsRef<Path>) -> Self {
        Self {
            state_file: path.as_ref().to_path_buf(),
        }
    }

    /// Save active persona name
    pub fn save(&self, name: &str) -> Result<()> {
        if let Some(parent) = self.state_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Internal(format!("Failed to create directory {:?}: {}", parent, e))
            })?;
        }

        std::fs::write(&self.state_file, name.to_lowercase()).map_err(|e| {
            Error::Internal(format!(
                "Failed to write active persona to {:?}: {}",
                self.state_file, e
            ))
        })
    }

    /// Load active persona name
    pub fn load(&self) -> Result<Option<String>> {
        if !self.state_file.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&self.state_file).map_err(|e| {
            Error::Internal(format!(
                "Failed to read active persona from {:?}: {}",
                self.state_file, e
            ))
        })?;

        let name = content.trim().to_string();
        if name.is_empty() {
            Ok(None)
        } else {
            Ok(Some(name))
        }
    }

    /// Clear active persona (dismiss)
    pub fn clear(&self) -> Result<()> {
        if self.state_file.exists() {
            std::fs::remove_file(&self.state_file).map_err(|e| {
                Error::Internal(format!(
                    "Failed to remove active persona file {:?}: {}",
                    self.state_file, e
                ))
            })?;
        }
        Ok(())
    }
}

impl Default for ActivePersonaState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;

