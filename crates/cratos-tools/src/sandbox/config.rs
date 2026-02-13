//! Sandbox configuration

use super::limits::ResourceLimits;
use super::policy::{NetworkMode, SandboxPolicy};
use super::runtime::ContainerRuntime;
use serde::{Deserialize, Serialize};
use tracing::warn;

/// Configuration for sandbox execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Sandbox policy
    pub policy: SandboxPolicy,
    /// Default network mode
    pub default_network: NetworkMode,
    /// Default resource limits
    pub default_limits: ResourceLimits,
    /// Container image to use for sandboxing
    pub image: String,
    /// Additional security options (Docker only)
    pub security_opts: Vec<String>,
    /// Path to seccomp profile JSON (Docker only)
    #[serde(default)]
    pub seccomp_profile: Option<std::path::PathBuf>,
    /// Preferred runtime: "auto", "docker", "apple_container", "none"
    #[serde(default = "default_runtime_preference")]
    pub runtime_preference: String,
    /// On macOS, prefer Apple Container over Docker when available
    #[serde(default = "default_true")]
    pub prefer_apple_container: bool,
}

fn default_runtime_preference() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for SandboxConfig {
    fn default() -> Self {
        // Check for bundled seccomp profile
        let seccomp_path = std::path::PathBuf::from("config/seccomp-default.json");
        let seccomp_profile = if seccomp_path.exists() {
            Some(seccomp_path)
        } else {
            None
        };

        let seccomp_opt = match &seccomp_profile {
            Some(path) => format!("seccomp={}", path.display()),
            None => {
                tracing::warn!(
                    "No seccomp profile found at config/seccomp-default.json, using unconfined"
                );
                "seccomp=unconfined".to_string()
            }
        };

        Self {
            policy: SandboxPolicy::Moderate,
            default_network: NetworkMode::None,
            default_limits: ResourceLimits::default(),
            image: "alpine:latest".to_string(),
            security_opts: vec!["no-new-privileges:true".to_string(), seccomp_opt],
            seccomp_profile,
            runtime_preference: "auto".to_string(),
            prefer_apple_container: true,
        }
    }
}

impl SandboxConfig {
    /// Select the container runtime based on configuration and availability
    pub async fn select_runtime(&self) -> ContainerRuntime {
        match self.runtime_preference.as_str() {
            "docker" => {
                if ContainerRuntime::check_docker().await {
                    ContainerRuntime::Docker
                } else {
                    warn!("Docker requested but not available");
                    ContainerRuntime::None
                }
            }
            "apple_container" => {
                if ContainerRuntime::check_apple_container().await {
                    ContainerRuntime::AppleContainer
                } else {
                    warn!("Apple Container requested but not available");
                    ContainerRuntime::None
                }
            }
            "none" => ContainerRuntime::None,
            _ => {
                // Auto-detect
                #[cfg(target_os = "macos")]
                {
                    if self.prefer_apple_container {
                        return ContainerRuntime::detect().await;
                    }
                }
                // Default: try Docker first
                if ContainerRuntime::check_docker().await {
                    ContainerRuntime::Docker
                } else {
                    ContainerRuntime::detect().await
                }
            }
        }
    }
}
