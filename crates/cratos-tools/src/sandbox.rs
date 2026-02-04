//! Docker Sandbox - Tool isolation using Docker containers
//!
//! This module provides secure sandboxing for tool execution using Docker containers.
//! Key features:
//! - Network isolation (default: none)
//! - Read-only filesystem mounts
//! - Resource limits (CPU, memory)
//! - Tool-specific isolation policies

#![forbid(unsafe_code)]

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, instrument, warn};

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
    /// Docker image to use for sandboxing
    pub image: String,
    /// Additional security options
    pub security_opts: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            policy: SandboxPolicy::Moderate,
            default_network: NetworkMode::None,
            default_limits: ResourceLimits::default(),
            image: "alpine:latest".to_string(),
            security_opts: vec![
                "no-new-privileges:true".to_string(),
                "seccomp=unconfined".to_string(), // TODO: Add custom seccomp profile
            ],
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
    }
}
