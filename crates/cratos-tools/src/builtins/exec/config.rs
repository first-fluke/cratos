/// Default maximum timeout in seconds
pub const DEFAULT_MAX_TIMEOUT_SECS: u64 = 60;

/// Default dangerous path patterns
pub const DEFAULT_BLOCKED_PATHS: &[&str] = &[
    "/etc",
    "/root",
    "/var/log",
    "/boot",
    "/dev",
    "/proc",
    "/sys",
    "/usr/bin",
    "/usr/sbin",
    "/bin",
    "/sbin",
];

/// Exec security mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMode {
    /// Block only built-in dangerous commands (default for personal machine)
    Permissive,
    /// Block all commands except those in `allowed_commands`
    Strict,
}

/// Execution host target
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecHost {
    /// Execute on the local machine (default)
    Local,
    /// Execute inside a Docker sandbox container
    Sandbox,
}

impl Default for ExecHost {
    fn default() -> Self {
        Self::Local
    }
}

/// Configuration for exec tool security
#[derive(Debug, Clone)]
pub struct ExecConfig {
    /// Security mode
    pub mode: ExecMode,
    /// Maximum timeout in seconds
    pub max_timeout_secs: u64,
    /// Additional commands to block (on top of built-in list)
    pub extra_blocked_commands: Vec<String>,
    /// Commands allowed when mode = Strict
    pub allowed_commands: Vec<String>,
    /// Blocked filesystem paths
    pub blocked_paths: Vec<String>,
    /// Allow network commands (curl, wget, etc.). Default: false.
    pub allow_network_commands: bool,
    /// Docker image for sandbox execution
    pub sandbox_image: String,
    /// Memory limit for sandbox containers (e.g. "256m")
    pub sandbox_memory_limit: String,
    /// CPU limit for sandbox containers (e.g. "1.0")
    pub sandbox_cpu_limit: String,
}

impl Default for ExecConfig {
    fn default() -> Self {
        Self {
            mode: ExecMode::Permissive,
            max_timeout_secs: DEFAULT_MAX_TIMEOUT_SECS,
            extra_blocked_commands: Vec::new(),
            allowed_commands: Vec::new(),
            blocked_paths: DEFAULT_BLOCKED_PATHS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            allow_network_commands: false,
            sandbox_image: "alpine:latest".to_string(),
            sandbox_memory_limit: "256m".to_string(),
            sandbox_cpu_limit: "1.0".to_string(),
        }
    }
}
