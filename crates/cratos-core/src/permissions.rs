//! Permissions - Tool and Channel Permission Management
//!
//! This module provides a flexible permission system for Cratos:
//! - Tool-based permissions (always allow, require confirmation, always deny)
//! - Channel-based permissions (different permissions for Telegram vs Slack)
//! - Time-based restrictions (block certain operations outside work hours)
//!
//! Unlike traditional user-based permissions, Cratos uses a personal assistant model
//! where permissions are scoped to tools, channels, and time rather than users.

use chrono::{Datelike, Local, NaiveTime};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Permission-related errors
#[derive(Debug, Error)]
pub enum PermissionError {
    /// Tool is explicitly denied
    #[error("Tool '{0}' is denied")]
    ToolDenied(String),

    /// Tool requires confirmation
    #[error("Tool '{0}' requires confirmation")]
    RequiresConfirmation(String),

    /// Tool is blocked in this channel
    #[error("Tool '{0}' is not allowed in channel '{1}'")]
    ChannelDenied(String, String),

    /// Tool is blocked outside work hours
    #[error("Tool '{0}' is blocked outside work hours")]
    TimeRestricted(String),

    /// Invalid time format
    #[error("Invalid time format: {0}")]
    InvalidTimeFormat(String),
}

/// Result type for permission operations
pub type Result<T> = std::result::Result<T, PermissionError>;

/// Permission check result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionStatus {
    /// Tool is allowed without confirmation
    Allowed,
    /// Tool requires user confirmation before execution
    RequiresConfirmation,
    /// Tool is denied (with reason)
    Denied(String),
}

impl PermissionStatus {
    /// Check if the permission allows execution
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed | Self::RequiresConfirmation)
    }

    /// Check if confirmation is required
    #[must_use]
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, Self::RequiresConfirmation)
    }

    /// Check if the tool is denied
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Denied(_))
    }
}

/// Tool permission configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermissions {
    /// Tools that are always allowed without confirmation
    #[serde(default)]
    pub always_allow: HashSet<String>,

    /// Tools that require user confirmation before execution
    #[serde(default)]
    pub require_confirmation: HashSet<String>,

    /// Tools that are always blocked
    #[serde(default)]
    pub always_deny: HashSet<String>,
}

impl Default for ToolPermissions {
    fn default() -> Self {
        let mut always_allow = HashSet::new();
        always_allow.insert("search".to_string());
        always_allow.insert("read_file".to_string());
        always_allow.insert("list_files".to_string());
        always_allow.insert("get_weather".to_string());

        let mut require_confirmation = HashSet::new();
        require_confirmation.insert("write_file".to_string());
        require_confirmation.insert("delete_file".to_string());
        require_confirmation.insert("shell".to_string());
        require_confirmation.insert("git_commit".to_string());
        require_confirmation.insert("git_push".to_string());

        let mut always_deny = HashSet::new();
        always_deny.insert("rm -rf".to_string());
        always_deny.insert("sudo".to_string());
        always_deny.insert("format".to_string());
        always_deny.insert("dd".to_string());

        Self {
            always_allow,
            require_confirmation,
            always_deny,
        }
    }
}

impl ToolPermissions {
    /// Check permission for a tool
    #[must_use]
    pub fn check(&self, tool_name: &str) -> PermissionStatus {
        // Check deny list first
        if self.always_deny.contains(tool_name) {
            return PermissionStatus::Denied(format!("Tool '{}' is explicitly denied", tool_name));
        }

        // Check for dangerous patterns
        let lower_name = tool_name.to_lowercase();
        for denied in &self.always_deny {
            if lower_name.contains(&denied.to_lowercase()) {
                return PermissionStatus::Denied(format!(
                    "Tool '{}' matches denied pattern '{}'",
                    tool_name, denied
                ));
            }
        }

        // Check confirmation required
        if self.require_confirmation.contains(tool_name) {
            return PermissionStatus::RequiresConfirmation;
        }

        // Check always allow
        if self.always_allow.contains(tool_name) {
            return PermissionStatus::Allowed;
        }

        // Default: require confirmation for unknown tools
        PermissionStatus::RequiresConfirmation
    }

    /// Add a tool to the allow list
    pub fn allow(&mut self, tool_name: impl Into<String>) {
        let name = tool_name.into();
        self.require_confirmation.remove(&name);
        self.always_deny.remove(&name);
        self.always_allow.insert(name);
    }

    /// Add a tool to the confirmation required list
    pub fn require_confirm(&mut self, tool_name: impl Into<String>) {
        let name = tool_name.into();
        self.always_allow.remove(&name);
        self.always_deny.remove(&name);
        self.require_confirmation.insert(name);
    }

    /// Add a tool to the deny list
    pub fn deny(&mut self, tool_name: impl Into<String>) {
        let name = tool_name.into();
        self.always_allow.remove(&name);
        self.require_confirmation.remove(&name);
        self.always_deny.insert(name);
    }
}

/// Channel-specific tool permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelToolConfig {
    /// Whether this channel is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Allowed tools (empty = all tools allowed)
    #[serde(default)]
    pub allow: HashSet<String>,

    /// Denied tools (takes precedence over allow)
    #[serde(default)]
    pub deny: HashSet<String>,
}

fn default_true() -> bool {
    true
}

impl Default for ChannelToolConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allow: HashSet::new(), // Empty = all allowed
            deny: HashSet::new(),
        }
    }
}

impl ChannelToolConfig {
    /// Create a permissive config (all tools allowed)
    #[must_use]
    pub fn permissive() -> Self {
        Self::default()
    }

    /// Create a restrictive config with specific allowed tools
    #[must_use]
    pub fn with_allowed(tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            enabled: true,
            allow: tools.into_iter().map(Into::into).collect(),
            deny: HashSet::new(),
        }
    }

    /// Check if a tool is allowed in this channel
    #[must_use]
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if !self.enabled {
            return false;
        }

        // Deny list takes precedence
        if self.deny.contains(tool_name) {
            return false;
        }

        // If allow list is empty, all tools are allowed
        if self.allow.is_empty() {
            return true;
        }

        // Check if tool is in allow list (support wildcards)
        if self.allow.contains("*") {
            return true;
        }

        self.allow.contains(tool_name)
    }
}

/// Channel permission configurations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelPermissions {
    /// Telegram channel permissions
    #[serde(default)]
    pub telegram: ChannelToolConfig,

    /// Slack channel permissions
    #[serde(default)]
    pub slack: ChannelToolConfig,

    /// CLI channel permissions
    #[serde(default)]
    pub cli: ChannelToolConfig,

    /// API channel permissions
    #[serde(default)]
    pub api: ChannelToolConfig,
}

impl ChannelPermissions {
    /// Get channel config by name
    #[must_use]
    pub fn get(&self, channel: &str) -> Option<&ChannelToolConfig> {
        match channel.to_lowercase().as_str() {
            "telegram" => Some(&self.telegram),
            "slack" => Some(&self.slack),
            "cli" | "terminal" => Some(&self.cli),
            "api" | "http" => Some(&self.api),
            _ => None,
        }
    }

    /// Check if a tool is allowed in a channel
    #[must_use]
    pub fn is_tool_allowed(&self, channel: &str, tool_name: &str) -> bool {
        self.get(channel)
            .map(|config| config.is_tool_allowed(tool_name))
            .unwrap_or(true) // Unknown channels default to allowed
    }
}

/// Time-based restrictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRestrictions {
    /// Whether time restrictions are enabled
    #[serde(default)]
    pub enabled: bool,

    /// Tools to block outside work hours
    #[serde(default)]
    pub off_hours_deny: HashSet<String>,

    /// Work hours start (e.g., "09:00")
    #[serde(default = "default_work_start")]
    pub work_start: String,

    /// Work hours end (e.g., "18:00")
    #[serde(default = "default_work_end")]
    pub work_end: String,

    /// Work days (0 = Sunday, 6 = Saturday)
    #[serde(default = "default_work_days")]
    pub work_days: Vec<u32>,
}

fn default_work_start() -> String {
    "09:00".to_string()
}

fn default_work_end() -> String {
    "18:00".to_string()
}

fn default_work_days() -> Vec<u32> {
    vec![1, 2, 3, 4, 5] // Monday to Friday
}

impl Default for TimeRestrictions {
    fn default() -> Self {
        Self {
            enabled: false,
            off_hours_deny: HashSet::new(),
            work_start: default_work_start(),
            work_end: default_work_end(),
            work_days: default_work_days(),
        }
    }
}

impl TimeRestrictions {
    /// Check if current time is within work hours
    #[must_use]
    pub fn is_work_hours(&self) -> bool {
        if !self.enabled {
            return true;
        }

        let now = Local::now();
        let current_time = now.time();
        let current_day = now.weekday().num_days_from_sunday();

        // Check if it's a work day
        if !self.work_days.contains(&current_day) {
            return false;
        }

        // Parse work hours
        let start = NaiveTime::parse_from_str(&self.work_start, "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        let end = NaiveTime::parse_from_str(&self.work_end, "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(18, 0, 0).unwrap());

        current_time >= start && current_time <= end
    }

    /// Check if a tool is allowed at current time
    #[must_use]
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if !self.enabled {
            return true;
        }

        // If the tool is in the off-hours deny list and it's outside work hours
        if self.off_hours_deny.contains(tool_name) && !self.is_work_hours() {
            return false;
        }

        true
    }
}

/// Complete permission configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    /// Permission mode: "permissive" or "strict"
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Tool-level permissions
    #[serde(default)]
    pub tools: ToolPermissions,

    /// Channel-level permissions
    #[serde(default)]
    pub channels: ChannelPermissions,

    /// Time-based restrictions
    #[serde(default)]
    pub time_restrictions: TimeRestrictions,
}

fn default_mode() -> String {
    "permissive".to_string()
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            tools: ToolPermissions::default(),
            channels: ChannelPermissions::default(),
            time_restrictions: TimeRestrictions::default(),
        }
    }
}

impl PermissionConfig {
    /// Create a permissive configuration
    #[must_use]
    pub fn permissive() -> Self {
        Self::default()
    }

    /// Create a strict configuration
    #[must_use]
    pub fn strict() -> Self {
        // In strict mode, all unknown tools require confirmation
        Self {
            mode: "strict".to_string(),
            tools: ToolPermissions {
                always_allow: HashSet::from([
                    "search".to_string(),
                    "list_files".to_string(),
                ]),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

/// Permission manager for checking tool access
pub struct PermissionManager {
    config: PermissionConfig,
}

impl PermissionManager {
    /// Create a new permission manager with the given configuration
    #[must_use]
    pub fn new(config: PermissionConfig) -> Self {
        Self { config }
    }

    /// Create with default (permissive) configuration
    #[must_use]
    pub fn permissive() -> Self {
        Self::new(PermissionConfig::permissive())
    }

    /// Create with strict configuration
    #[must_use]
    pub fn strict() -> Self {
        Self::new(PermissionConfig::strict())
    }

    /// Check permission for a tool in a channel
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool to check
    /// * `channel` - Channel name (e.g., "telegram", "slack", "cli")
    ///
    /// # Returns
    /// `PermissionStatus` indicating whether the tool is allowed, requires confirmation, or denied
    #[must_use]
    pub fn check(&self, tool_name: &str, channel: &str) -> PermissionStatus {
        // Check time restrictions first
        if !self.config.time_restrictions.is_tool_allowed(tool_name) {
            return PermissionStatus::Denied(format!(
                "Tool '{}' is blocked outside work hours",
                tool_name
            ));
        }

        // Check channel permissions
        if !self.config.channels.is_tool_allowed(channel, tool_name) {
            return PermissionStatus::Denied(format!(
                "Tool '{}' is not allowed in channel '{}'",
                tool_name, channel
            ));
        }

        // Check tool permissions
        self.config.tools.check(tool_name)
    }

    /// Check if a tool can be executed (allowed or requires confirmation)
    #[must_use]
    pub fn can_execute(&self, tool_name: &str, channel: &str) -> bool {
        self.check(tool_name, channel).is_allowed()
    }

    /// Check if a tool requires confirmation
    #[must_use]
    pub fn requires_confirmation(&self, tool_name: &str, channel: &str) -> bool {
        self.check(tool_name, channel).requires_confirmation()
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &PermissionConfig {
        &self.config
    }

    /// Get a mutable reference to the configuration
    pub fn config_mut(&mut self) -> &mut PermissionConfig {
        &mut self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_permissions_default() {
        let perms = ToolPermissions::default();

        // Always allowed tools
        assert_eq!(perms.check("search"), PermissionStatus::Allowed);
        assert_eq!(perms.check("read_file"), PermissionStatus::Allowed);

        // Require confirmation tools
        assert_eq!(
            perms.check("write_file"),
            PermissionStatus::RequiresConfirmation
        );
        assert_eq!(perms.check("shell"), PermissionStatus::RequiresConfirmation);

        // Always denied tools
        assert!(perms.check("rm -rf").is_denied());
        assert!(perms.check("sudo").is_denied());

        // Unknown tools require confirmation
        assert_eq!(
            perms.check("unknown_tool"),
            PermissionStatus::RequiresConfirmation
        );
    }

    #[test]
    fn test_tool_permissions_deny_patterns() {
        let perms = ToolPermissions::default();

        // Tools containing denied patterns should be blocked
        assert!(perms.check("sudo rm").is_denied());
        assert!(perms.check("rm -rf /").is_denied());
    }

    #[test]
    fn test_tool_permissions_modification() {
        let mut perms = ToolPermissions::default();

        // Allow a previously denied tool
        perms.allow("shell");
        assert_eq!(perms.check("shell"), PermissionStatus::Allowed);

        // Deny a previously allowed tool
        perms.deny("search");
        assert!(perms.check("search").is_denied());
    }

    #[test]
    fn test_channel_permissions() {
        let mut channels = ChannelPermissions::default();

        // Configure Slack with restricted tools
        channels.slack = ChannelToolConfig::with_allowed(["search", "read_file"]);
        channels.slack.deny.insert("shell".to_string());

        // Telegram uses default (all allowed)
        assert!(channels.is_tool_allowed("telegram", "shell"));
        assert!(channels.is_tool_allowed("telegram", "write_file"));

        // Slack has restrictions
        assert!(channels.is_tool_allowed("slack", "search"));
        assert!(!channels.is_tool_allowed("slack", "shell"));
        assert!(!channels.is_tool_allowed("slack", "write_file"));
    }

    #[test]
    fn test_permission_manager() {
        let manager = PermissionManager::permissive();

        // Check tool in telegram (permissive channel)
        assert_eq!(
            manager.check("search", "telegram"),
            PermissionStatus::Allowed
        );
        assert_eq!(
            manager.check("write_file", "telegram"),
            PermissionStatus::RequiresConfirmation
        );
        assert!(manager.check("sudo", "telegram").is_denied());
    }

    #[test]
    fn test_permission_status() {
        assert!(PermissionStatus::Allowed.is_allowed());
        assert!(PermissionStatus::RequiresConfirmation.is_allowed());
        assert!(!PermissionStatus::Denied("reason".to_string()).is_allowed());

        assert!(!PermissionStatus::Allowed.requires_confirmation());
        assert!(PermissionStatus::RequiresConfirmation.requires_confirmation());
        assert!(!PermissionStatus::Denied("reason".to_string()).requires_confirmation());
    }

    #[test]
    fn test_strict_mode() {
        let manager = PermissionManager::strict();

        // In strict mode, fewer tools are allowed by default
        assert_eq!(
            manager.check("search", "cli"),
            PermissionStatus::Allowed
        );
        // read_file should require confirmation in strict mode
        assert_eq!(
            manager.check("read_file", "cli"),
            PermissionStatus::RequiresConfirmation
        );
    }
}
