//! Security analysis for bash commands
//!
//! Implements Layer 2 (Pipeline Analysis) and Layer 3 (Environment/Path Isolation)

use super::config::{BashConfig, BashSecurityMode};
use super::constants::*;
use crate::error::{Error, Result};
use std::path::PathBuf;
use tracing::warn;

/// Check if a string contains shell glob metacharacters.
pub(crate) fn contains_glob_chars(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '*' | '?' | '[' | ']'))
}

/// Check if an exit code 1 is informational (not a real failure) for the given command.
pub fn is_informational_exit(command: &str, exit_code: i32) -> bool {
    if exit_code != 1 {
        return false;
    }
    // Check the last segment of a pipeline (e.g., `ps aux | grep node` → check `grep`)
    let last_segment = command.split('|').next_back().unwrap_or(command).trim();
    let first_cmd = last_segment.split_whitespace().next().unwrap_or("");
    let base = first_cmd.rsplit('/').next().unwrap_or(first_cmd);
    INFORMATIONAL_EXIT_COMMANDS.contains(&base)
}

/// Security analyzer for bash commands
pub(crate) struct SecurityAnalyzer<'a> {
    config: &'a BashConfig,
}

impl<'a> SecurityAnalyzer<'a> {
    pub fn new(config: &'a BashConfig) -> Self {
        Self { config }
    }

    /// Layer 2: Pipeline Analysis
    pub fn analyze_pipeline(&self, command: &str) -> Result<()> {
        // C1: Block process substitution (can bypass all checks)
        if command.contains("<(") || command.contains(">(") {
            return Err(Error::PermissionDenied(
                "Process substitution is not allowed".into(),
            ));
        }

        // H2: Block heredoc (<<) but NOT append redirect (>>)
        // Look for "<<" that is NOT preceded by ">"
        {
            let chars: Vec<char> = command.chars().collect();
            let len = chars.len();
            for i in 0..len.saturating_sub(1) {
                if chars[i] == '<' && chars[i + 1] == '<' {
                    // Check this is not inside ">>" → i.e. preceded by '>'
                    if i == 0 || chars[i - 1] != '>' {
                        return Err(Error::PermissionDenied(
                            "Heredoc (<<) is not allowed".into(),
                        ));
                    }
                }
            }
        }

        // Split by pipe and check each segment
        for segment in command.split('|') {
            let trimmed = segment.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Handle command chains (&&, ||, ;)
            for sub in trimmed.split(&['&', ';'][..]) {
                let sub = sub.trim();
                if sub.is_empty() {
                    continue;
                }

                // Extract the first token (command name)
                let first_token = sub.split_whitespace().next().unwrap_or("");
                // Strip path: /usr/bin/rm → rm
                let base_cmd = first_token.split('/').next_back().unwrap_or(first_token);

                // H3: Block glob characters in command token (not in args)
                if contains_glob_chars(base_cmd) {
                    warn!(command = %base_cmd, "Glob pattern in command token blocked");
                    return Err(Error::PermissionDenied(format!(
                        "Glob patterns in command names are not allowed: '{}'",
                        base_cmd
                    )));
                }

                // H4: Block alias/function definitions (injection vectors)
                if base_cmd == "alias" {
                    // Allow "alias" (list all) and "alias -p", block "alias x=..."
                    if sub.contains('=') {
                        warn!("Alias definition blocked");
                        return Err(Error::PermissionDenied(
                            "Alias definitions are not allowed".into(),
                        ));
                    }
                }
                if base_cmd == "function" {
                    warn!("Function definition blocked");
                    return Err(Error::PermissionDenied(
                        "Function definitions are not allowed".into(),
                    ));
                }

                if self.is_command_blocked(base_cmd) {
                    warn!(command = %base_cmd, "Blocked command in pipeline");
                    return Err(Error::PermissionDenied(format!(
                        "Command '{}' is blocked for security reasons",
                        base_cmd
                    )));
                }
            }
        }

        // V6: Check redirection targets
        self.check_redirections(command)?;

        // Check dangerous patterns
        self.check_dangerous_patterns(command)?;

        Ok(())
    }

    pub fn is_command_blocked(&self, cmd: &str) -> bool {
        match self.config.security_mode {
            BashSecurityMode::Strict => !self.config.allowed_commands.iter().any(|a| a == cmd),
            BashSecurityMode::Permissive => {
                let builtin_blocked = BLOCKED_COMMANDS.contains(&cmd);
                let network_blocked =
                    !self.config.allow_network_commands && NETWORK_EXFIL_COMMANDS.contains(&cmd);
                let extra_blocked = self.config.blocked_commands.iter().any(|b| b == cmd);
                // C2: Block versioned interpreters (e.g. python3.11, perl5.34)
                let prefix_blocked = BLOCKED_COMMAND_PREFIXES.iter().any(|p| cmd.starts_with(p));
                // "." is an alias for "source"
                builtin_blocked || network_blocked || extra_blocked || prefix_blocked || cmd == "."
            }
        }
    }

    fn check_dangerous_patterns(&self, command: &str) -> Result<()> {
        for pattern in DANGEROUS_PATTERNS {
            if command.contains(pattern) {
                warn!(pattern = %pattern, "Dangerous pattern detected");
                return Err(Error::PermissionDenied(format!(
                    "Command contains dangerous pattern: '{}'",
                    pattern
                )));
            }
        }
        Ok(())
    }

    /// Check redirection targets against blocked paths (V6).
    fn check_redirections(&self, command: &str) -> Result<()> {
        let chars: Vec<char> = command.chars().collect();
        let len = chars.len();
        let mut i = 0;
        while i < len {
            // Skip quoted strings
            if chars[i] == '\'' || chars[i] == '"' {
                let q = chars[i];
                i += 1;
                while i < len && chars[i] != q {
                    if chars[i] == '\\' && q == '"' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                continue;
            }
            // Detect > or N> (e.g. 2>)
            let is_redir = chars[i] == '>'
                || (i + 1 < len && chars[i].is_ascii_digit() && chars[i + 1] == '>');
            if is_redir {
                while i < len && (chars[i] == '>' || chars[i].is_ascii_digit()) {
                    i += 1;
                }
                while i < len && chars[i] == ' ' {
                    i += 1;
                }
                let start = i;
                while i < len && !chars[i].is_whitespace() && !matches!(chars[i], '|' | ';' | '&') {
                    i += 1;
                }
                if start < i {
                    let target: String = chars[start..i].iter().collect();
                    // C3: Block variable expansion in redirection targets
                    if target.contains('$') || target.contains('`') {
                        return Err(Error::PermissionDenied(
                            "Variable expansion in redirection target is not allowed".into(),
                        ));
                    }
                    // Allow /dev/null (safe discard target) even though /dev is blocked
                    if target != "/dev/null" {
                        for blocked in &self.config.blocked_paths {
                            if target.starts_with(blocked.as_str()) {
                                return Err(Error::PermissionDenied(format!(
                                    "Redirection to restricted path '{}' blocked",
                                    target
                                )));
                            }
                        }
                    }
                }
            } else {
                i += 1;
            }
        }
        // Block archiving sensitive directories
        let sensitive = ["~/.ssh", "~/.gnupg", "~/.aws", "~/.docker", "~/.kube"];
        if ["tar", "zip", "7z"].iter().any(|c| command.contains(c)) {
            for s in &sensitive {
                if command.contains(s) {
                    return Err(Error::PermissionDenied(format!(
                        "Archiving sensitive directory '{}' blocked",
                        s
                    )));
                }
            }
        }
        Ok(())
    }

    /// Validate send_keys input against blocked commands and dangerous patterns.
    /// Prevents injection attacks through interactive sessions (V2).
    pub fn validate_send_keys(&self, keys: &str) -> Result<()> {
        let processed = keys
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
            .replace("\\x03", "\x03")
            .replace("\\x04", "\x04")
            .replace("\\x1a", "\x1a");

        for line in processed.split('\n') {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Allow single control characters (Ctrl+C, Ctrl+D, etc.)
            if trimmed.len() == 1 && trimmed.as_bytes()[0] < 0x20 {
                continue;
            }
            // Short single word (≤10 chars, no spaces) — only check block list
            if trimmed.len() <= 10 && !trimmed.contains(' ') {
                let base = trimmed.split('/').next_back().unwrap_or(trimmed);
                if self.is_command_blocked(base) {
                    return Err(Error::PermissionDenied(format!(
                        "send_keys: blocked command '{}' detected",
                        base
                    )));
                }
                continue;
            }
            // Multi-word input → full pipeline analysis
            self.analyze_pipeline(trimmed)?;
        }
        Ok(())
    }

    /// Layer 3: Validate working directory
    pub fn validate_cwd(&self, cwd: &str) -> Result<PathBuf> {
        let path = PathBuf::from(cwd);

        // Check blocked paths
        let path_str = path.to_string_lossy();
        for blocked in &self.config.blocked_paths {
            if path_str.starts_with(blocked.as_str()) {
                return Err(Error::PermissionDenied(format!(
                    "Working directory '{}' is restricted",
                    cwd
                )));
            }
        }

        // Workspace jail check
        if self.config.workspace_jail {
            if let Some(workspace) = &self.config.default_cwd {
                let canonical_cwd = std::fs::canonicalize(&path).map_err(|e| {
                    Error::InvalidInput(format!(
                        "Cannot resolve working directory '{}': {}",
                        cwd, e
                    ))
                })?;
                let canonical_workspace = std::fs::canonicalize(workspace).map_err(|e| {
                    Error::InvalidInput(format!(
                        "Cannot resolve workspace '{}': {}",
                        workspace.display(),
                        e
                    ))
                })?;
                if !canonical_cwd.starts_with(&canonical_workspace) {
                    return Err(Error::PermissionDenied(format!(
                        "Working directory '{}' is outside workspace '{}'",
                        cwd,
                        workspace.display()
                    )));
                }
            }
        }

        Ok(path)
    }

    /// Build environment whitelist
    pub fn build_env_whitelist(&self) -> Vec<(String, String)> {
        self.config
            .env_whitelist
            .iter()
            .filter_map(|key| std::env::var(key).ok().map(|val| (key.clone(), val)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> BashConfig {
        BashConfig::default()
    }

    #[test]
    fn test_blocked_command_in_pipeline() {
        let config = default_config();
        let analyzer = SecurityAnalyzer::new(&config);
        // rm in pipeline should be blocked
        assert!(analyzer.analyze_pipeline("echo hi | rm -rf /").is_err());
        // sudo should be blocked
        assert!(analyzer.analyze_pipeline("sudo ls").is_err());
        // eval should be blocked
        assert!(analyzer.analyze_pipeline("eval 'echo test'").is_err());
        // safe pipeline should pass
        assert!(analyzer
            .analyze_pipeline("ps aux | grep node | head -20")
            .is_ok());
        // chained safe commands
        assert!(analyzer.analyze_pipeline("echo hello && ls -la").is_ok());
    }

    #[test]
    fn test_dangerous_pattern_ld_preload() {
        let config = default_config();
        let analyzer = SecurityAnalyzer::new(&config);
        assert!(analyzer.analyze_pipeline("LD_PRELOAD=/evil.so ls").is_err());
        assert!(analyzer
            .analyze_pipeline("DYLD_INSERT_LIBRARIES=/x ls")
            .is_err());
    }

    #[test]
    fn test_workspace_jail_escape() {
        let config = BashConfig {
            workspace_jail: true,
            default_cwd: Some(PathBuf::from("/tmp/workspace")),
            ..BashConfig::default()
        };
        let analyzer = SecurityAnalyzer::new(&config);
        // Attempt to escape workspace
        assert!(analyzer.validate_cwd("/etc").is_err());
    }

    #[test]
    fn test_env_whitelist() {
        let config = default_config();
        let analyzer = SecurityAnalyzer::new(&config);
        let env_vars = analyzer.build_env_whitelist();
        // Should only contain whitelisted variables
        for (key, _) in &env_vars {
            assert!(
                ENV_WHITELIST.contains(&key.as_str()),
                "Unexpected env var: {}",
                key
            );
        }
    }

    #[test]
    fn test_blocked_path() {
        let config = default_config();
        let analyzer = SecurityAnalyzer::new(&config);
        assert!(analyzer.validate_cwd("/etc").is_err());
        assert!(analyzer.validate_cwd("/root").is_err());
        assert!(analyzer.validate_cwd("/tmp").is_ok());
    }

    #[test]
    fn test_strict_mode() {
        let config = BashConfig {
            security_mode: BashSecurityMode::Strict,
            allowed_commands: vec!["ls".to_string(), "cat".to_string()],
            ..BashConfig::default()
        };
        let analyzer = SecurityAnalyzer::new(&config);
        // Only allowed commands pass
        assert!(analyzer.analyze_pipeline("ls -la").is_ok());
        assert!(analyzer.analyze_pipeline("cat /tmp/test").is_ok());
        // Everything else blocked
        assert!(analyzer.analyze_pipeline("echo hello").is_err());
        assert!(analyzer.analyze_pipeline("git status").is_err());
    }

    #[test]
    fn test_informational_exit_codes() {
        // grep exit 1 = no match → informational
        assert!(is_informational_exit("grep pattern file.txt", 1));
        // Pipeline: last command is grep
        assert!(is_informational_exit("ps aux | grep nonexistent", 1));
        // diff exit 1 = files differ → informational
        assert!(is_informational_exit("diff a.txt b.txt", 1));
        // Non-informational command with exit 1 → NOT informational
        assert!(!is_informational_exit("ls /nonexistent", 1));
    }
}
