//! Container Sandbox - Tool isolation using container runtimes
//!
//! This module provides secure sandboxing for tool execution using container runtimes.
//! Supports multiple runtimes:
//! - Docker: Cross-platform, process-based isolation
//! - Apple Container: macOS 26+ native, VM-based isolation (stronger security)
//!
//! Key features:
//! - Network isolation (default: none)
//! - Read-only filesystem mounts
//! - Resource limits (CPU, memory)
//! - Tool-specific isolation policies
//! - Automatic runtime detection

#![forbid(unsafe_code)]

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, instrument, warn};

// ============================================================================
// Container Runtime
// ============================================================================

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
    async fn check_apple_container() -> bool {
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
    async fn check_docker() -> bool {
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

// ============================================================================
// Sandbox Policy
// ============================================================================

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

// ============================================================================
// Network Mode
// ============================================================================

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

// ============================================================================
// Resource Limits
// ============================================================================

/// Resource limits for sandboxed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Memory limit in bytes (default: 512MB)
    pub memory_bytes: u64,
    /// CPU quota (percentage of one core, default: 50%)
    pub cpu_percent: u32,
    /// Maximum execution time
    pub timeout: Duration,
    /// Maximum number of processes
    pub max_pids: u32,
    /// Disable swap
    pub no_swap: bool,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            memory_bytes: 512 * 1024 * 1024, // 512MB
            cpu_percent: 50,
            timeout: Duration::from_secs(60),
            max_pids: 100,
            no_swap: true,
        }
    }
}

impl ResourceLimits {
    /// Create resource limits with custom memory
    #[must_use]
    pub fn with_memory_mb(mut self, mb: u64) -> Self {
        self.memory_bytes = mb * 1024 * 1024;
        self
    }

    /// Create resource limits with custom CPU
    #[must_use]
    pub fn with_cpu_percent(mut self, percent: u32) -> Self {
        self.cpu_percent = percent.min(100);
        self
    }

    /// Create resource limits with custom timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Convert to Docker resource arguments
    #[must_use]
    pub fn to_docker_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Memory limit
        args.push(format!("--memory={}b", self.memory_bytes));

        // Disable swap if requested
        if self.no_swap {
            args.push(format!("--memory-swap={}b", self.memory_bytes));
        }

        // CPU quota (in microseconds per 100ms period)
        let cpu_quota = (self.cpu_percent as u64) * 1000;
        args.push(format!("--cpu-quota={}", cpu_quota));
        args.push("--cpu-period=100000".to_string());

        // PID limit
        args.push(format!("--pids-limit={}", self.max_pids));

        args
    }

    /// Convert to Apple Container resource arguments
    #[must_use]
    pub fn to_apple_container_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Memory limit (in MB for Apple Container)
        let memory_mb = self.memory_bytes / (1024 * 1024);
        args.push(format!("--memory={}m", memory_mb));

        // CPU cores (Apple Container uses cores, not percentage)
        // Convert percentage to a fraction of a core
        let cpu_cores = (self.cpu_percent as f32 / 100.0).max(0.1);
        args.push(format!("--cpus={:.1}", cpu_cores));

        args
    }
}

// ============================================================================
// Mount Configuration
// ============================================================================

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

// ============================================================================
// Sandbox Configuration
// ============================================================================

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

// ============================================================================
// Docker Sandbox
// ============================================================================

/// Docker sandbox for isolated tool execution
///
/// This struct provides Docker-based sandboxing for executing untrusted tools
/// in an isolated environment. Currently a work-in-progress feature.
///
/// TODO: Complete Docker sandbox implementation
#[allow(dead_code)] // Work-in-progress feature
pub struct DockerSandbox {
    config: SandboxConfig,
    /// Container ID if running
    container_id: Option<String>,
}

impl DockerSandbox {
    /// Create a new Docker sandbox
    #[must_use]
    pub fn new(config: SandboxConfig) -> Self {
        Self {
            config,
            container_id: None,
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(SandboxConfig::default())
    }

    /// Get the sandbox configuration
    #[must_use]
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }

    /// Check if Docker is available
    pub async fn is_available(&self) -> bool {
        match tokio::process::Command::new("docker")
            .arg("info")
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Execute a command in a sandboxed container
    #[instrument(skip(self, command, env), fields(image = %self.config.image))]
    pub async fn execute(
        &self,
        command: &[String],
        env: HashMap<String, String>,
        mounts: Vec<Mount>,
        network: Option<NetworkMode>,
        limits: Option<ResourceLimits>,
    ) -> Result<SandboxOutput> {
        let network = network.unwrap_or(self.config.default_network);
        let limits = limits.unwrap_or_else(|| self.config.default_limits.clone());

        // Build docker command
        let mut docker_args = vec![
            "run".to_string(),
            "--rm".to_string(),
            format!("--network={}", network.as_docker_arg()),
        ];

        // Add resource limits
        docker_args.extend(limits.to_docker_args());

        // Add security options
        for opt in &self.config.security_opts {
            docker_args.push(format!("--security-opt={}", opt));
        }

        // Add environment variables
        for (key, value) in &env {
            // Sanitize environment variable names
            if Self::is_valid_env_name(key) {
                docker_args.push("-e".to_string());
                docker_args.push(format!("{}={}", key, value));
            } else {
                warn!(key = %key, "Skipping invalid environment variable name");
            }
        }

        // Add mounts
        for mount in &mounts {
            docker_args.push(mount.to_docker_arg());
        }

        // Add image
        docker_args.push(self.config.image.clone());

        // Add command
        docker_args.extend(command.iter().cloned());

        debug!(args = ?docker_args, "Executing sandboxed command");

        // Execute with timeout
        let output = tokio::time::timeout(
            limits.timeout,
            tokio::process::Command::new("docker")
                .args(&docker_args)
                .output(),
        )
        .await
        .map_err(|_| Error::Timeout(limits.timeout.as_millis() as u64))?
        .map_err(|e| Error::Execution(format!("Docker execution failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        info!(
            exit_code = exit_code,
            stdout_len = stdout.len(),
            stderr_len = stderr.len(),
            "Sandbox execution completed"
        );

        Ok(SandboxOutput {
            stdout,
            stderr,
            exit_code,
            success: output.status.success(),
        })
    }

    /// Execute a shell script in sandbox
    pub async fn execute_script(
        &self,
        script: &str,
        env: HashMap<String, String>,
        mounts: Vec<Mount>,
    ) -> Result<SandboxOutput> {
        let command = vec!["/bin/sh".to_string(), "-c".to_string(), script.to_string()];

        self.execute(&command, env, mounts, None, None).await
    }

    /// Validate environment variable name
    fn is_valid_env_name(name: &str) -> bool {
        !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.chars().next().unwrap_or('0').is_ascii_digit()
    }
}

// ============================================================================
// Sandbox Output
// ============================================================================

/// Output from sandboxed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxOutput {
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Exit code
    pub exit_code: i32,
    /// Whether execution was successful
    pub success: bool,
}

impl SandboxOutput {
    /// Create a successful output
    #[must_use]
    pub fn success(stdout: impl Into<String>) -> Self {
        Self {
            stdout: stdout.into(),
            stderr: String::new(),
            exit_code: 0,
            success: true,
        }
    }

    /// Create a failed output
    #[must_use]
    pub fn failure(stderr: impl Into<String>, exit_code: i32) -> Self {
        Self {
            stdout: String::new(),
            stderr: stderr.into(),
            exit_code,
            success: false,
        }
    }

    /// Get combined output
    #[must_use]
    pub fn combined_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }
}

// ============================================================================
// Tool Sandbox Wrapper
// ============================================================================

/// Wrapper for executing tools with optional sandboxing
pub struct ToolSandbox {
    docker: DockerSandbox,
    enabled: bool,
}

impl ToolSandbox {
    /// Create a new tool sandbox
    #[must_use]
    pub fn new(config: SandboxConfig) -> Self {
        let enabled = config.policy != SandboxPolicy::Disabled;
        Self {
            docker: DockerSandbox::new(config),
            enabled,
        }
    }

    /// Check if sandboxing is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Should this tool be sandboxed?
    #[must_use]
    pub fn should_sandbox(&self, risk_level: &str) -> bool {
        self.enabled && self.docker.config.policy.should_sandbox(risk_level)
    }

    /// Get the underlying Docker sandbox
    #[must_use]
    pub fn docker(&self) -> &DockerSandbox {
        &self.docker
    }
}

// ============================================================================
// Unified Sandbox (supports Docker and Apple Container)
// ============================================================================

/// Unified sandbox that automatically selects the best available runtime
///
/// On macOS 26+ with Apple Silicon, prefers Apple Container for stronger
/// VM-based isolation. Falls back to Docker on other platforms.
pub struct UnifiedSandbox {
    config: SandboxConfig,
    runtime: ContainerRuntime,
}

impl UnifiedSandbox {
    /// Create a new unified sandbox with automatic runtime detection
    pub async fn new(config: SandboxConfig) -> Self {
        let runtime = config.select_runtime().await;
        info!(
            runtime = %runtime.display_name(),
            vm_isolated = runtime.is_vm_isolated(),
            "Sandbox initialized"
        );
        Self { config, runtime }
    }

    /// Create with a specific runtime (for testing)
    #[must_use]
    pub fn with_runtime(config: SandboxConfig, runtime: ContainerRuntime) -> Self {
        Self { config, runtime }
    }

    /// Get the current container runtime
    #[must_use]
    pub fn runtime(&self) -> ContainerRuntime {
        self.runtime
    }

    /// Get the sandbox configuration
    #[must_use]
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }

    /// Check if sandboxing is available
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.runtime != ContainerRuntime::None
    }

    /// Execute a command in a sandboxed container
    #[instrument(skip(self, command, env), fields(runtime = %self.runtime.display_name(), image = %self.config.image))]
    pub async fn execute(
        &self,
        command: &[String],
        env: HashMap<String, String>,
        mounts: Vec<Mount>,
        network: Option<NetworkMode>,
        limits: Option<ResourceLimits>,
    ) -> Result<SandboxOutput> {
        match self.runtime {
            ContainerRuntime::AppleContainer => {
                self.execute_apple_container(command, env, mounts, network, limits)
                    .await
            }
            ContainerRuntime::Docker => {
                self.execute_docker(command, env, mounts, network, limits)
                    .await
            }
            ContainerRuntime::None => {
                // No sandboxing available - execute natively with caution
                warn!("No sandbox available - executing without isolation");
                self.execute_native(command, env).await
            }
        }
    }

    /// Execute using Apple Container (macOS 26+)
    async fn execute_apple_container(
        &self,
        command: &[String],
        env: HashMap<String, String>,
        mounts: Vec<Mount>,
        network: Option<NetworkMode>,
        limits: Option<ResourceLimits>,
    ) -> Result<SandboxOutput> {
        let network = network.unwrap_or(self.config.default_network);
        let limits = limits.unwrap_or_else(|| self.config.default_limits.clone());

        let mut args = vec![
            "run".to_string(),
            "--rm".to_string(),
            format!("--network={}", network.as_apple_container_arg()),
        ];

        // Add resource limits
        args.extend(limits.to_apple_container_args());

        // Add environment variables
        for (key, value) in &env {
            if Self::is_valid_env_name(key) {
                args.push("--env".to_string());
                args.push(format!("{}={}", key, value));
            } else {
                warn!(key = %key, "Skipping invalid environment variable name");
            }
        }

        // Add mounts
        for mount in &mounts {
            args.push(mount.to_apple_container_arg());
        }

        // Add image
        args.push(self.config.image.clone());

        // Add command
        args.extend(command.iter().cloned());

        debug!(args = ?args, "Executing with Apple Container");

        // Execute with timeout
        let output = tokio::time::timeout(
            limits.timeout,
            tokio::process::Command::new("container")
                .args(&args)
                .output(),
        )
        .await
        .map_err(|_| Error::Timeout(limits.timeout.as_millis() as u64))?
        .map_err(|e| Error::Execution(format!("Apple Container execution failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        info!(
            exit_code = exit_code,
            stdout_len = stdout.len(),
            stderr_len = stderr.len(),
            runtime = "apple_container",
            "Sandbox execution completed"
        );

        Ok(SandboxOutput {
            stdout,
            stderr,
            exit_code,
            success: output.status.success(),
        })
    }

    /// Execute using Docker
    async fn execute_docker(
        &self,
        command: &[String],
        env: HashMap<String, String>,
        mounts: Vec<Mount>,
        network: Option<NetworkMode>,
        limits: Option<ResourceLimits>,
    ) -> Result<SandboxOutput> {
        let network = network.unwrap_or(self.config.default_network);
        let limits = limits.unwrap_or_else(|| self.config.default_limits.clone());

        let mut docker_args = vec![
            "run".to_string(),
            "--rm".to_string(),
            format!("--network={}", network.as_docker_arg()),
        ];

        // Add resource limits
        docker_args.extend(limits.to_docker_args());

        // Add security options
        for opt in &self.config.security_opts {
            docker_args.push(format!("--security-opt={}", opt));
        }

        // Add environment variables
        for (key, value) in &env {
            if Self::is_valid_env_name(key) {
                docker_args.push("-e".to_string());
                docker_args.push(format!("{}={}", key, value));
            } else {
                warn!(key = %key, "Skipping invalid environment variable name");
            }
        }

        // Add mounts
        for mount in &mounts {
            docker_args.push(mount.to_docker_arg());
        }

        // Add image
        docker_args.push(self.config.image.clone());

        // Add command
        docker_args.extend(command.iter().cloned());

        debug!(args = ?docker_args, "Executing with Docker");

        // Execute with timeout
        let output = tokio::time::timeout(
            limits.timeout,
            tokio::process::Command::new("docker")
                .args(&docker_args)
                .output(),
        )
        .await
        .map_err(|_| Error::Timeout(limits.timeout.as_millis() as u64))?
        .map_err(|e| Error::Execution(format!("Docker execution failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        info!(
            exit_code = exit_code,
            stdout_len = stdout.len(),
            stderr_len = stderr.len(),
            runtime = "docker",
            "Sandbox execution completed"
        );

        Ok(SandboxOutput {
            stdout,
            stderr,
            exit_code,
            success: output.status.success(),
        })
    }

    /// Execute without sandbox (fallback when no runtime is available)
    async fn execute_native(
        &self,
        command: &[String],
        env: HashMap<String, String>,
    ) -> Result<SandboxOutput> {
        if command.is_empty() {
            return Err(Error::Execution("Empty command".to_string()));
        }

        let mut cmd = tokio::process::Command::new(&command[0]);
        if command.len() > 1 {
            cmd.args(&command[1..]);
        }

        for (key, value) in &env {
            if Self::is_valid_env_name(key) {
                cmd.env(key, value);
            }
        }

        let output = cmd
            .output()
            .await
            .map_err(|e| Error::Execution(format!("Native execution failed: {}", e)))?;

        Ok(SandboxOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            success: output.status.success(),
        })
    }

    /// Validate environment variable name
    fn is_valid_env_name(name: &str) -> bool {
        !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.chars().next().unwrap_or('0').is_ascii_digit()
    }

    /// Execute a shell script in sandbox
    pub async fn execute_script(
        &self,
        script: &str,
        env: HashMap<String, String>,
        mounts: Vec<Mount>,
    ) -> Result<SandboxOutput> {
        let command = vec!["/bin/sh".to_string(), "-c".to_string(), script.to_string()];
        self.execute(&command, env, mounts, None, None).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_policy() {
        assert!(SandboxPolicy::Strict.should_sandbox("low"));
        assert!(SandboxPolicy::Strict.should_sandbox("high"));

        assert!(!SandboxPolicy::Moderate.should_sandbox("low"));
        assert!(SandboxPolicy::Moderate.should_sandbox("medium"));
        assert!(SandboxPolicy::Moderate.should_sandbox("high"));

        assert!(!SandboxPolicy::Disabled.should_sandbox("high"));
    }

    #[test]
    fn test_network_mode() {
        assert_eq!(NetworkMode::None.as_docker_arg(), "none");
        assert_eq!(NetworkMode::Bridge.as_docker_arg(), "bridge");
        assert_eq!(NetworkMode::Host.as_docker_arg(), "host");
    }

    #[test]
    fn test_resource_limits() {
        let limits = ResourceLimits::default()
            .with_memory_mb(256)
            .with_cpu_percent(25)
            .with_timeout(Duration::from_secs(30));

        assert_eq!(limits.memory_bytes, 256 * 1024 * 1024);
        assert_eq!(limits.cpu_percent, 25);
        assert_eq!(limits.timeout, Duration::from_secs(30));

        let args = limits.to_docker_args();
        assert!(args.iter().any(|a| a.contains("--memory=")));
        assert!(args.iter().any(|a| a.contains("--cpu-quota=")));
        assert!(args.iter().any(|a| a.contains("--pids-limit=")));
    }

    #[test]
    fn test_mount() {
        let ro_mount = Mount::read_only("/host/path", "/container/path");
        assert!(ro_mount.read_only);
        assert!(ro_mount.to_docker_arg().contains("readonly"));

        let rw_mount = Mount::read_write("/host/data", "/data");
        assert!(!rw_mount.read_only);
        assert!(!rw_mount.to_docker_arg().contains("readonly"));
    }

    #[test]
    fn test_valid_env_name() {
        assert!(DockerSandbox::is_valid_env_name("PATH"));
        assert!(DockerSandbox::is_valid_env_name("MY_VAR"));
        assert!(DockerSandbox::is_valid_env_name("VAR123"));

        assert!(!DockerSandbox::is_valid_env_name(""));
        assert!(!DockerSandbox::is_valid_env_name("123VAR"));
        assert!(!DockerSandbox::is_valid_env_name("MY-VAR"));
        assert!(!DockerSandbox::is_valid_env_name("MY VAR"));
    }

    #[test]
    fn test_sandbox_output() {
        let success = SandboxOutput::success("output");
        assert!(success.success);
        assert_eq!(success.exit_code, 0);

        let failure = SandboxOutput::failure("error", 1);
        assert!(!failure.success);
        assert_eq!(failure.exit_code, 1);
    }

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert_eq!(config.policy, SandboxPolicy::Moderate);
        assert_eq!(config.default_network, NetworkMode::None);
        assert_eq!(config.runtime_preference, "auto");
        assert!(config.prefer_apple_container);
    }

    #[test]
    fn test_container_runtime_display_name() {
        assert_eq!(ContainerRuntime::Docker.display_name(), "Docker");
        assert_eq!(
            ContainerRuntime::AppleContainer.display_name(),
            "Apple Container"
        );
        assert_eq!(ContainerRuntime::None.display_name(), "None (no isolation)");
    }

    #[test]
    fn test_container_runtime_vm_isolation() {
        assert!(!ContainerRuntime::Docker.is_vm_isolated());
        assert!(ContainerRuntime::AppleContainer.is_vm_isolated());
        assert!(!ContainerRuntime::None.is_vm_isolated());
    }

    #[test]
    fn test_network_mode_apple_container_arg() {
        assert_eq!(NetworkMode::None.as_apple_container_arg(), "none");
        assert_eq!(NetworkMode::Bridge.as_apple_container_arg(), "nat");
        assert_eq!(NetworkMode::Host.as_apple_container_arg(), "host");
    }

    #[test]
    fn test_mount_apple_container_arg() {
        let ro_mount = Mount::read_only("/host/path", "/container/path");
        let arg = ro_mount.to_apple_container_arg();
        assert!(arg.contains("--mount="));
        assert!(arg.contains(":ro"));

        let rw_mount = Mount::read_write("/host/data", "/data");
        let arg = rw_mount.to_apple_container_arg();
        assert!(!arg.contains(":ro"));
    }

    #[test]
    fn test_resource_limits_apple_container_args() {
        let limits = ResourceLimits::default()
            .with_memory_mb(256)
            .with_cpu_percent(50);

        let args = limits.to_apple_container_args();
        assert!(args.iter().any(|a| a.contains("--memory=")));
        assert!(args.iter().any(|a| a.contains("--cpus=")));
    }

    #[test]
    fn test_unified_sandbox_with_runtime() {
        let config = SandboxConfig::default();

        let docker_sandbox = UnifiedSandbox::with_runtime(config.clone(), ContainerRuntime::Docker);
        assert_eq!(docker_sandbox.runtime(), ContainerRuntime::Docker);
        assert!(docker_sandbox.is_available());

        let apple_sandbox =
            UnifiedSandbox::with_runtime(config.clone(), ContainerRuntime::AppleContainer);
        assert_eq!(apple_sandbox.runtime(), ContainerRuntime::AppleContainer);
        assert!(apple_sandbox.is_available());

        let none_sandbox = UnifiedSandbox::with_runtime(config, ContainerRuntime::None);
        assert_eq!(none_sandbox.runtime(), ContainerRuntime::None);
        assert!(!none_sandbox.is_available());
    }

    #[test]
    fn test_unified_sandbox_is_valid_env_name() {
        assert!(UnifiedSandbox::is_valid_env_name("PATH"));
        assert!(UnifiedSandbox::is_valid_env_name("MY_VAR"));
        assert!(UnifiedSandbox::is_valid_env_name("VAR123"));

        assert!(!UnifiedSandbox::is_valid_env_name(""));
        assert!(!UnifiedSandbox::is_valid_env_name("123VAR"));
        assert!(!UnifiedSandbox::is_valid_env_name("MY-VAR"));
    }
}
