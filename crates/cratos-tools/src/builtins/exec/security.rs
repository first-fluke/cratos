use std::path::PathBuf;
use super::config::{ExecMode, ExecConfig};

/// Shell metacharacters that indicate command injection attempts
pub const SHELL_METACHARACTERS: &[char] = &[
    '|', ';', '&', '$', '`', '(', ')', '<', '>', '\n', '\r', '!', '#',
];

/// Default dangerous commands that are always blocked
pub const DEFAULT_BLOCKED_COMMANDS: &[&str] = &[
    "rm", "rmdir", "dd", "mkfs", "fdisk", "parted", "shred", "truncate",
    "shutdown", "reboot", "poweroff", "halt", "init",
    "passwd", "useradd", "userdel", "usermod", "groupadd", "groupdel", "chmod", "chown", "chgrp",
    "iptables", "ip6tables", "nft",
    "bash", "sh", "zsh", "fish", "csh", "tcsh", "ksh",
    "nc", "netcat", "ncat",
    "sudo", "su", "doas",
    "docker", "podman", "kubectl", "crictl",
    "kill", "pkill", "killall",
    "crontab", "at", "launchctl", "systemctl",
    "ln",
    "python", "python3", "perl", "ruby", "node", "php", "lua", "tclsh", "wish",
    "env", "xargs", "nice", "timeout", "watch", "strace", "ltrace", "nohup", "setsid", "osascript",
];

pub const BLOCKED_COMMAND_PREFIXES: &[&str] = &[
    "python", "perl", "ruby", "node", "php", "lua", "tclsh", "wish",
];

pub const NETWORK_EXFIL_COMMANDS: &[&str] = &[
    "curl", "wget", "scp", "sftp", "rsync", "ftp", "telnet", "socat", "ssh",
];

pub fn contains_shell_metacharacters(s: &str) -> Option<char> {
    s.chars().find(|&c| SHELL_METACHARACTERS.contains(&c))
}

pub fn is_command_blocked(config: &ExecConfig, command: &str) -> bool {
    let base_command = command
        .split('/')
        .next_back()
        .unwrap_or(command)
        .split_whitespace()
        .next()
        .unwrap_or(command);

    match config.mode {
        ExecMode::Strict => {
            !config.allowed_commands.iter().any(|a| a == base_command)
        }
        ExecMode::Permissive => {
            let is_builtin_blocked = DEFAULT_BLOCKED_COMMANDS.contains(&base_command);
            let is_network_blocked = !config.allow_network_commands
                && NETWORK_EXFIL_COMMANDS.contains(&base_command);
            let is_extra_blocked = config.extra_blocked_commands.iter().any(|b| b == base_command);
            let is_prefix_blocked = BLOCKED_COMMAND_PREFIXES.iter().any(|p| base_command.starts_with(p));
            is_builtin_blocked || is_network_blocked || is_extra_blocked || is_prefix_blocked
        }
    }
}

pub fn is_path_dangerous(config: &ExecConfig, path: &str) -> bool {
    let normalized = PathBuf::from(path);
    let path_str = normalized.to_string_lossy();
    config.blocked_paths.iter().any(|pattern| path_str.starts_with(pattern.as_str()))
}
