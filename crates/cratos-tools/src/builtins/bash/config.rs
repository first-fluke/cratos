//! Bash tool configuration types

use super::constants::*;
use std::path::PathBuf;

/// Security mode for the bash tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BashSecurityMode {
    /// Block only built-in dangerous commands (default for personal machine).
    Permissive,
    /// Block all commands except those in `allowed_commands`.
    Strict,
}

impl Default for BashSecurityMode {
    fn default() -> Self {
        Self::Permissive
    }
}

/// Configuration for the bash tool.
#[derive(Debug, Clone)]
pub struct BashConfig {
    /// Security mode.
    pub security_mode: BashSecurityMode,
    /// Default timeout in seconds.
    pub default_timeout_secs: u64,
    /// Maximum timeout in seconds.
    pub max_timeout_secs: u64,
    /// Maximum concurrent sessions.
    pub max_sessions: usize,
    /// Maximum output size in bytes.
    pub max_output_bytes: usize,
    /// Additional commands to block.
    pub blocked_commands: Vec<String>,
    /// Commands allowed when mode = Strict.
    pub allowed_commands: Vec<String>,
    /// Blocked path patterns.
    pub blocked_paths: Vec<String>,
    /// Workspace directory (jail root).
    pub default_cwd: Option<PathBuf>,
    /// Enable workspace jail (prevent cwd escape).
    pub workspace_jail: bool,
    /// Environment variable whitelist.
    pub env_whitelist: Vec<String>,
    /// Maximum commands per minute (rate limit).
    pub max_commands_per_minute: u32,
    /// Allow network commands (curl, wget, etc.). Default: false.
    pub allow_network_commands: bool,
}

impl Default for BashConfig {
    fn default() -> Self {
        Self {
            security_mode: BashSecurityMode::default(),
            default_timeout_secs: DEFAULT_TIMEOUT_SECS,
            max_timeout_secs: MAX_TIMEOUT_SECS,
            max_sessions: MAX_SESSIONS,
            max_output_bytes: MAX_OUTPUT_BYTES,
            blocked_commands: Vec::new(),
            allowed_commands: Vec::new(),
            blocked_paths: vec![
                "/etc".into(),
                "/root".into(),
                "/var/log".into(),
                "/boot".into(),
                "/dev".into(),
                "/proc".into(),
                "/sys".into(),
            ],
            default_cwd: None,
            workspace_jail: false,
            env_whitelist: ENV_WHITELIST.iter().map(|s| (*s).to_string()).collect(),
            max_commands_per_minute: MAX_COMMANDS_PER_MINUTE,
            allow_network_commands: false,
        }
    }
}
