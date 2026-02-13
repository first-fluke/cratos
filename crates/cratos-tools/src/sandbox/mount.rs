//! Mount point configuration for sandboxed containers

use serde::{Deserialize, Serialize};

/// Mount point for sandboxed container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mount {
    /// Host path
    pub source: String,
    /// Container path
    pub target: String,
    /// Read-only mount
    pub read_only: bool,
}

impl Mount {
    /// Create a new read-only mount
    #[must_use]
    pub fn read_only(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            read_only: true,
        }
    }

    /// Create a new read-write mount
    #[must_use]
    pub fn read_write(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            read_only: false,
        }
    }

    /// Convert to Docker mount argument
    #[must_use]
    pub fn to_docker_arg(&self) -> String {
        let ro = if self.read_only { ",readonly" } else { "" };
        format!(
            "--mount=type=bind,source={},target={}{}",
            self.source, self.target, ro
        )
    }

    /// Convert to Apple Container mount argument
    #[must_use]
    pub fn to_apple_container_arg(&self) -> String {
        let ro = if self.read_only { ":ro" } else { "" };
        format!("--mount={}:{}{}", self.source, self.target, ro)
    }
}
