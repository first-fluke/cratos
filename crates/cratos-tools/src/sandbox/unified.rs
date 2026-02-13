//! Unified sandbox supporting Docker and Apple Container

use super::config::SandboxConfig;
use super::docker::DockerSandbox;
use super::limits::ResourceLimits;
use super::mount::Mount;
use super::output::SandboxOutput;
use super::policy::{NetworkMode, SandboxPolicy};
use super::runtime::ContainerRuntime;
use crate::error::{Error, Result};
use std::collections::HashMap;
use tracing::{debug, info, instrument, warn};

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
        self.enabled && self.docker.config().policy.should_sandbox(risk_level)
    }

    /// Get the underlying Docker sandbox
    #[must_use]
    pub fn docker(&self) -> &DockerSandbox {
        &self.docker
    }
}

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
