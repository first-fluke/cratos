//! Sandbox policy and network mode configuration

use serde::{Deserialize, Serialize};

/// Sandbox policy determines which tools are sandboxed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxPolicy {
    /// Sandbox all tool executions
    Strict,
    /// Sandbox only dangerous tools (default)
    #[default]
    Moderate,
    /// No sandboxing (development only)
    Disabled,
}

impl SandboxPolicy {
    /// Check if a tool should be sandboxed based on its risk level
    #[must_use]
    pub fn should_sandbox(&self, risk_level: &str) -> bool {
        match self {
            Self::Strict => true,
            Self::Moderate => matches!(risk_level, "high" | "medium"),
            Self::Disabled => false,
        }
    }
}

/// Network mode for sandboxed execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkMode {
    /// No network access (most secure, default)
    #[default]
    None,
    /// Bridge network (isolated network)
    Bridge,
    /// Host network (least secure, not recommended)
    Host,
}

impl NetworkMode {
    /// Convert to Docker network mode string
    #[must_use]
    pub fn as_docker_arg(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bridge => "bridge",
            Self::Host => "host",
        }
    }

    /// Convert to Apple Container network mode string
    #[must_use]
    pub fn as_apple_container_arg(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bridge => "nat", // Apple Container uses "nat" for isolated networking
            Self::Host => "host",
        }
    }
}
