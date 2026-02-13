//! Resource limits for sandboxed execution

use serde::{Deserialize, Serialize};
use std::time::Duration;

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
