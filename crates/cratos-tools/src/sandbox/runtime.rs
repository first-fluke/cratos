//! Container Runtime detection and management

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Supported container runtimes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerRuntime {
    /// Docker - Cross-platform, process-based isolation
    Docker,
    /// Apple Container - macOS 26+ native, VM-based isolation (stronger)
    AppleContainer,
    /// No container runtime available
    None,
}

impl ContainerRuntime {
    /// Detect the best available container runtime
    ///
    /// On macOS with Apple Silicon, prefers Apple Container if available.
    /// Falls back to Docker, then None.
    pub async fn detect() -> Self {
        // On macOS with Apple Silicon, try Apple Container first
        #[cfg(target_os = "macos")]
        {
            if Self::is_apple_silicon() && Self::check_apple_container().await {
                info!("Using Apple Container runtime (VM-based isolation)");
                return Self::AppleContainer;
            }
        }

        // Try Docker
        if Self::check_docker().await {
            info!("Using Docker runtime");
            return Self::Docker;
        }

        warn!("No container runtime available - sandboxing disabled");
        Self::None
    }

    /// Check if Apple Container CLI is available
    pub async fn check_apple_container() -> bool {
        match tokio::process::Command::new("container")
            .arg("--version")
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Check if Docker is available
    pub async fn check_docker() -> bool {
        match tokio::process::Command::new("docker")
            .arg("info")
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Check if running on Apple Silicon
    #[cfg(target_os = "macos")]
    fn is_apple_silicon() -> bool {
        std::env::consts::ARCH == "aarch64"
    }

    /// Get human-readable name for the runtime
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Docker => "Docker",
            Self::AppleContainer => "Apple Container",
            Self::None => "None (no isolation)",
        }
    }

    /// Check if this runtime provides VM-level isolation
    #[must_use]
    pub fn is_vm_isolated(&self) -> bool {
        matches!(self, Self::AppleContainer)
    }
}
