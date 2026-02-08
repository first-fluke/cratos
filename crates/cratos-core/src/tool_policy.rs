//! Tool Policy — command allowlist/denylist for safe remote execution.
//!
//! Follows the OpenClaw pattern of dual-gating:
//! 1. Platform-level denylist (always blocks dangerous commands)
//! 2. Node-declared command list (node must declare what it can run)

use serde::{Deserialize, Serialize};

/// Reason a command was denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDenial {
    /// Command is in the global deny list
    DenyListed(String),
    /// Command is not in the node's declared commands
    NotDeclared(String),
}

impl std::fmt::Display for PolicyDenial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DenyListed(cmd) => write!(f, "command '{}' is deny-listed", cmd),
            Self::NotDeclared(cmd) => write!(f, "command '{}' not declared by node", cmd),
        }
    }
}

impl std::error::Error for PolicyDenial {}

/// Tool execution policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicy {
    /// Commands that are always blocked (deny takes precedence)
    #[serde(default = "default_deny_commands")]
    pub deny_commands: Vec<String>,
    /// Platform-specific default allowlists
    #[serde(default)]
    pub platform_defaults: PlatformDefaults,
}

/// Per-platform command defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformDefaults {
    /// Allowed commands on macOS
    #[serde(default = "default_darwin_commands")]
    pub darwin: Vec<String>,
    /// Allowed commands on Linux
    #[serde(default = "default_linux_commands")]
    pub linux: Vec<String>,
}

impl Default for PlatformDefaults {
    fn default() -> Self {
        Self {
            darwin: default_darwin_commands(),
            linux: default_linux_commands(),
        }
    }
}

fn default_deny_commands() -> Vec<String> {
    vec![
        "rm -rf /".to_string(),
        "dd".to_string(),
        "mkfs".to_string(),
        "shutdown".to_string(),
        "reboot".to_string(),
        "halt".to_string(),
        "init 0".to_string(),
        "init 6".to_string(),
        ":(){:|:&};:".to_string(), // fork bomb
    ]
}

fn default_darwin_commands() -> Vec<String> {
    vec![
        "bash".to_string(),
        "sh".to_string(),
        "python3".to_string(),
        "git".to_string(),
        "cargo".to_string(),
        "npm".to_string(),
        "brew".to_string(),
        "ls".to_string(),
        "cat".to_string(),
        "grep".to_string(),
        "find".to_string(),
    ]
}

fn default_linux_commands() -> Vec<String> {
    vec![
        "bash".to_string(),
        "sh".to_string(),
        "python3".to_string(),
        "git".to_string(),
        "cargo".to_string(),
        "npm".to_string(),
        "docker".to_string(),
        "systemctl".to_string(),
        "ls".to_string(),
        "cat".to_string(),
        "grep".to_string(),
        "find".to_string(),
    ]
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self {
            deny_commands: default_deny_commands(),
            platform_defaults: PlatformDefaults::default(),
        }
    }
}

impl ToolPolicy {
    /// Check if a command is allowed given the policy and node's declared commands.
    ///
    /// Dual gate:
    /// 1. Not in deny list
    /// 2. In node's declared commands
    pub fn is_allowed(
        &self,
        command: &str,
        node_declared_commands: &[String],
    ) -> Result<(), PolicyDenial> {
        let cmd_lower = command.to_lowercase();

        // Gate 1: Deny list (always takes precedence)
        for deny in &self.deny_commands {
            if cmd_lower.contains(&deny.to_lowercase()) {
                return Err(PolicyDenial::DenyListed(deny.clone()));
            }
        }

        // Gate 2: Extract the base command (first token)
        let base_cmd = command.split_whitespace().next().unwrap_or(command);

        // Check if base command is in node's declared commands
        if !node_declared_commands.iter().any(|d| d == base_cmd) {
            return Err(PolicyDenial::NotDeclared(base_cmd.to_string()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deny_list_blocks() {
        let policy = ToolPolicy::default();
        let declared = vec!["rm".to_string()];

        let result = policy.is_allowed("rm -rf /", &declared);
        assert!(matches!(result, Err(PolicyDenial::DenyListed(_))));
    }

    #[test]
    fn test_undeclared_command_blocked() {
        let policy = ToolPolicy::default();
        let declared = vec!["git".to_string(), "cargo".to_string()];

        let result = policy.is_allowed("npm install", &declared);
        assert!(matches!(result, Err(PolicyDenial::NotDeclared(_))));
    }

    #[test]
    fn test_declared_command_allowed() {
        let policy = ToolPolicy::default();
        let declared = vec!["git".to_string(), "cargo".to_string()];

        assert!(policy.is_allowed("git status", &declared).is_ok());
        assert!(policy.is_allowed("cargo build", &declared).is_ok());
    }

    #[test]
    fn test_deny_overrides_declared() {
        let policy = ToolPolicy::default();
        // Even if node declares "dd", deny list blocks it
        let declared = vec!["dd".to_string()];

        let result = policy.is_allowed("dd if=/dev/zero", &declared);
        assert!(matches!(result, Err(PolicyDenial::DenyListed(_))));
    }

    #[test]
    fn test_fork_bomb_blocked() {
        let policy = ToolPolicy::default();
        let declared = vec!["bash".to_string()];

        let result = policy.is_allowed(":(){:|:&};:", &declared);
        assert!(matches!(result, Err(PolicyDenial::DenyListed(_))));
    }

    #[test]
    fn test_empty_declared_blocks_all() {
        let policy = ToolPolicy::default();
        let declared: Vec<String> = vec![];

        let result = policy.is_allowed("ls", &declared);
        assert!(matches!(result, Err(PolicyDenial::NotDeclared(_))));
    }

    #[test]
    fn test_default_policy() {
        let policy = ToolPolicy::default();
        assert!(!policy.deny_commands.is_empty());
        assert!(!policy.platform_defaults.darwin.is_empty());
        assert!(!policy.platform_defaults.linux.is_empty());
    }
}
