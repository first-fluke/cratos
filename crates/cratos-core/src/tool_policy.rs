//! Tool Policy — command allowlist/denylist for safe remote execution.
//!
//! Follows the pattern of dual-gating:
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

// ────────────────────────────────────────────────────────────────────
// 6-Level Hierarchical Tool Security Policy
// ────────────────────────────────────────────────────────────────────
//
// Resolution order (most specific wins):
//   1. Sandbox — per-execution-environment (Docker, local)
//   2. Group   — per-tool-group (filesystem, network, system)
//   3. Agent   — per-persona (@sindri, @athena)
//   4. Global  — site-wide default
//   5. Provider — per-LLM-provider (gemini, openai)
//   6. Profile — per-user profile

/// What a policy rule says about a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    /// Tool call is allowed without approval
    Allow,
    /// Tool call is denied unconditionally
    Deny,
    /// Tool call requires human approval before execution
    RequireApproval,
}

/// The six policy levels, ordered from most specific to least specific.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyLevel {
    /// Per-execution sandbox environment (Docker, local)
    Sandbox,
    /// Per-tool group (filesystem, network, system)
    Group,
    /// Per-persona / agent
    Agent,
    /// Site-wide global default
    Global,
    /// Per-LLM provider
    Provider,
    /// Per-user profile
    Profile,
}

impl PolicyLevel {
    /// Priority (lower = more specific = wins).
    fn priority(self) -> u8 {
        match self {
            Self::Sandbox => 0,
            Self::Group => 1,
            Self::Agent => 2,
            Self::Global => 3,
            Self::Provider => 4,
            Self::Profile => 5,
        }
    }
}

/// A single policy rule entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Which policy level this rule lives at
    pub level: PolicyLevel,
    /// Scope qualifier (e.g. "docker" for Sandbox, "filesystem" for Group, "@sindri" for Agent)
    pub scope: String,
    /// Tool name pattern (glob-like: "*" matches all, "exec" matches exact, "file_*" matches prefix)
    pub tool_pattern: String,
    /// Action to take
    pub action: PolicyAction,
}

/// 6-level hierarchical tool security policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolSecurityPolicy {
    /// All registered policy rules
    rules: Vec<PolicyRule>,
}

/// Context for resolving which policy rules apply.
#[derive(Debug, Clone, Default)]
pub struct PolicyContext {
    /// Current sandbox environment (e.g. "docker", "local")
    pub sandbox: Option<String>,
    /// Tool's group (e.g. "filesystem", "network", "system")
    pub tool_group: Option<String>,
    /// Active persona / agent (e.g. "@sindri")
    pub agent: Option<String>,
    /// LLM provider name (e.g. "gemini")
    pub provider: Option<String>,
    /// User profile name
    pub profile: Option<String>,
}

impl ToolSecurityPolicy {
    /// Create a new empty policy.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule to the policy.
    pub fn add_rule(&mut self, rule: PolicyRule) {
        self.rules.push(rule);
    }

    /// Resolve the effective policy action for a given tool in a given context.
    ///
    /// Returns the most-specific matching rule's action, or `None` if no rule matches.
    pub fn resolve(&self, tool_name: &str, ctx: &PolicyContext) -> Option<PolicyAction> {
        let mut best: Option<(u8, PolicyAction)> = None;

        for rule in &self.rules {
            if !matches_pattern(&rule.tool_pattern, tool_name) {
                continue;
            }
            if !matches_context(rule, ctx) {
                continue;
            }
            let pri = rule.level.priority();
            if best.is_none() || pri < best.unwrap().0 {
                best = Some((pri, rule.action));
            }
        }

        best.map(|(_, action)| action)
    }

    /// Resolve with a fallback default (Allow if nothing matches).
    pub fn resolve_or_default(&self, tool_name: &str, ctx: &PolicyContext) -> PolicyAction {
        self.resolve(tool_name, ctx).unwrap_or(PolicyAction::Allow)
    }

    /// List all rules.
    pub fn rules(&self) -> &[PolicyRule] {
        &self.rules
    }

    /// Create a policy with sensible defaults:
    /// - Global: all tools allowed
    /// - Global: exec/bash require approval
    /// - Sandbox(docker): all tools allowed (sandboxed environment)
    pub fn with_defaults() -> Self {
        let mut policy = Self::new();
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Global,
            scope: "*".to_string(),
            tool_pattern: "*".to_string(),
            action: PolicyAction::Allow,
        });
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Global,
            scope: "*".to_string(),
            tool_pattern: "exec".to_string(),
            action: PolicyAction::RequireApproval,
        });
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Global,
            scope: "*".to_string(),
            tool_pattern: "bash".to_string(),
            action: PolicyAction::RequireApproval,
        });
        policy.add_rule(PolicyRule {
            level: PolicyLevel::Sandbox,
            scope: "docker".to_string(),
            tool_pattern: "*".to_string(),
            action: PolicyAction::Allow,
        });
        policy
    }
}

/// Check if a tool name matches a pattern (simple glob: "*" = all, "foo*" = prefix, exact otherwise)
fn matches_pattern(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    pattern == name
}

/// Check if a rule's level/scope matches the given context.
fn matches_context(rule: &PolicyRule, ctx: &PolicyContext) -> bool {
    match rule.level {
        PolicyLevel::Sandbox => ctx
            .sandbox
            .as_ref()
            .is_some_and(|s| rule.scope == "*" || s == &rule.scope),
        PolicyLevel::Group => ctx
            .tool_group
            .as_ref()
            .is_some_and(|g| rule.scope == "*" || g == &rule.scope),
        PolicyLevel::Agent => ctx
            .agent
            .as_ref()
            .is_some_and(|a| rule.scope == "*" || a == &rule.scope),
        PolicyLevel::Global => rule.scope == "*",
        PolicyLevel::Provider => ctx
            .provider
            .as_ref()
            .is_some_and(|p| rule.scope == "*" || p == &rule.scope),
        PolicyLevel::Profile => ctx
            .profile
            .as_ref()
            .is_some_and(|p| rule.scope == "*" || p == &rule.scope),
    }
}

#[cfg(test)]
mod tests;

