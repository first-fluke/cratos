//! Git Security Utilities
//!
//! Security validation functions for git operations.

/// Dangerous git flags that should be blocked
///
/// These flags can cause destructive operations or bypass safety checks:
/// - Force push/delete flags that can overwrite history
/// - Hooks bypass flags that skip safety validations
/// - Mirror/prune flags that can delete remote branches
#[cfg(test)]
pub const BLOCKED_FLAGS: &[&str] = &[
    "--force",
    "-f",
    "--force-with-lease",
    "--no-verify",
    "-n",
    "--hard",
    "--delete",
    "-D",
    "--force-delete",
    "--mirror",
    "--prune",
    "--all", // when used with push
];

/// Check if an argument contains blocked flags
///
/// Returns true if the argument matches any blocked flag exactly or as a prefix
/// (e.g., `--force=true` matches `--force`).
#[cfg(test)]
pub fn contains_blocked_flag(arg: &str) -> bool {
    BLOCKED_FLAGS
        .iter()
        .any(|flag| arg == *flag || arg.starts_with(&format!("{}=", flag)))
}

/// Validate branch name for security (prevent command injection)
pub fn is_valid_branch_name(name: &str) -> bool {
    // Git branch names have specific restrictions
    // Reject anything that looks like it could be command injection
    if name.is_empty() || name.len() > 255 {
        return false;
    }

    // Must not start with - (could be interpreted as flag)
    if name.starts_with('-') {
        return false;
    }

    // Must not contain dangerous characters
    let dangerous_chars = ['`', '$', '|', ';', '&', '>', '<', '\n', '\r', '\0'];
    if name.chars().any(|c| dangerous_chars.contains(&c)) {
        return false;
    }

    // Must not contain .. (path traversal)
    if name.contains("..") {
        return false;
    }

    true
}

/// Validate a git clone URL for security
pub fn is_valid_clone_url(url: &str) -> bool {
    // Must start with a safe protocol
    let safe_protocols = ["https://", "git://", "ssh://", "git@"];
    if !safe_protocols.iter().any(|p| url.starts_with(p)) {
        return false;
    }

    // Block shell injection characters
    let dangerous_chars = [
        '`', '$', '|', ';', '&', '>', '<', '\n', '\r', '\0', '\'', '"',
    ];
    if url.chars().any(|c| dangerous_chars.contains(&c)) {
        return false;
    }

    // Block javascript: and other dangerous pseudo-protocols
    let lower = url.to_lowercase();
    if lower.starts_with("javascript:") || lower.starts_with("data:") {
        return false;
    }

    true
}

/// Validate a local path for security (no path traversal)
pub fn is_valid_clone_path(path: &str) -> bool {
    if path.is_empty() || path.len() > 4096 {
        return false;
    }

    // Block path traversal
    if path.contains("..") {
        return false;
    }

    // Block shell injection characters
    let dangerous_chars = ['`', '$', '|', ';', '&', '>', '<', '\n', '\r', '\0'];
    if path.chars().any(|c| dangerous_chars.contains(&c)) {
        return false;
    }

    true
}
