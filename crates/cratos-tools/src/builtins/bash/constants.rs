//! Constants for bash tool security layers

// ── Time/Size Constants ────────────────────────────────────────────────────

pub const DEFAULT_TIMEOUT_SECS: u64 = 120;
pub const MAX_TIMEOUT_SECS: u64 = 1800;
pub const MAX_SESSIONS: usize = 4;
pub const MAX_OUTPUT_BYTES: usize = 100 * 1024; // 100 KB
pub const MAX_COMMANDS_PER_MINUTE: u32 = 30;
pub const SESSION_IDLE_TIMEOUT_SECS: u64 = 1800; // 30 minutes
pub const SESSION_CLEANUP_INTERVAL_SECS: u64 = 60;

// ── Blocked Commands ───────────────────────────────────────────────────────

/// Commands blocked in pipeline segments.
pub const BLOCKED_COMMANDS: &[&str] = &[
    // Destructive system commands
    "rm",
    "rmdir",
    "dd",
    "mkfs",
    "fdisk",
    "parted",
    "shred",
    "truncate",
    // System control
    "shutdown",
    "reboot",
    "poweroff",
    "halt",
    "init",
    // User/permission manipulation
    "passwd",
    "useradd",
    "userdel",
    "usermod",
    "chmod",
    "chown",
    "chgrp",
    // Firewall
    "iptables",
    "ip6tables",
    "nft",
    // Network tools (raw socket)
    "nc",
    "netcat",
    "ncat",
    // Privilege escalation
    "sudo",
    "su",
    "doas",
    // Shell-specific dangers
    "eval",
    "source",
    "exec",
    "nohup",
    "disown",
    "setsid",
    "chroot",
    "unshare",
    "nsenter",
    // Container/VM escape
    "docker",
    "podman",
    "kubectl",
    "crictl",
    // Process control
    "kill",
    "pkill",
    "killall",
    // Persistence mechanisms
    "crontab",
    "at",
    "launchctl",
    "systemctl",
    // Symlink attacks (V5)
    "ln",
    // Interpreters (can bypass all checks)
    "python",
    "python3",
    "perl",
    "ruby",
    "node",
    "php",
    "lua",
    "tclsh",
    "wish",
];

/// Network exfiltration commands blocked by default.
/// Users needing HTTP should use http_get/http_post tools.
pub const NETWORK_EXFIL_COMMANDS: &[&str] = &[
    "curl", "wget", "scp", "sftp", "rsync", "ftp", "telnet", "socat", "ssh",
];

/// Dangerous patterns in command strings (environment injection, remote code exec, etc.)
pub const DANGEROUS_PATTERNS: &[&str] = &[
    "LD_PRELOAD=",
    "LD_LIBRARY_PATH=",
    "DYLD_INSERT_LIBRARIES=",
    ">/dev/sda",
    "/dev/mem",
    "mkfifo",
    "$(curl",
    "$(wget",
    "`curl",
    "`wget",
    "base64 -d",
    // V7: Encoding bypass patterns
    "| base64",
    "| xxd",
    "| openssl enc",
];

/// Environment variables allowed in PTY sessions.
pub const ENV_WHITELIST: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LANG",
    "LC_ALL",
    "TERM",
    "TMPDIR",
    "XDG_RUNTIME_DIR",
    "SHELL",
];

/// Command prefixes blocked to prevent versioned interpreter bypass (e.g. `python3.11`, `perl5.34`).
pub const BLOCKED_COMMAND_PREFIXES: &[&str] = &[
    "python", "perl", "ruby", "node", "php", "lua", "tclsh", "wish",
];

/// Patterns indicating secrets in output.
pub const SECRET_PATTERNS: &[&str] = &[
    "BEGIN RSA PRIVATE KEY",
    "BEGIN OPENSSH PRIVATE KEY",
    "BEGIN PGP PRIVATE KEY",
    "PRIVATE KEY-----",
    "AKIA", // AWS access key
    "aws_secret_access_key",
    "sk-",         // OpenAI
    "ghp_",        // GitHub
    "gho_",        // GitHub OAuth
    "glpat-",      // GitLab
    "xoxb-",       // Slack bot
    "xoxp-",       // Slack personal
    "postgres://", // DB URLs
    "mysql://",
    "mongodb://",
];

/// Commands where exit code 1 is informational (not an error).
/// - grep/rg/ag: exit 1 = "no match found"
/// - diff/cmp: exit 1 = "files differ"
/// - test/[: exit 1 = "condition false"
/// - which/command: exit 1 = "not found"
/// - lsof: exit 1 = "no matches"
pub const INFORMATIONAL_EXIT_COMMANDS: &[&str] = &[
    "grep", "egrep", "fgrep", "rg", "ag", "diff", "cmp", "test", "[", "which", "command", "lsof",
];
