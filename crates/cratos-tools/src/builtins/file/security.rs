use crate::error::{Error, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tracing::{debug, warn};

/// Sensitive file patterns that require extra caution
static SENSITIVE_FILE_PATTERNS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        ".env",
        ".env.local",
        ".env.production",
        ".env.development",
        "credentials",
        "credentials.json",
        "secrets",
        "secret",
        ".aws",
        ".ssh",
        "id_rsa",
        "id_ed25519",
        ".gitconfig",
        ".npmrc",
        ".pypirc",
        "token",
        "api_key",
        "apikey",
        "password",
        ".htpasswd",
        "shadow",
        "passwd",
    ])
});

/// Blocked directory patterns
static BLOCKED_DIRECTORIES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
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
        "/var/run",
        "/run",
    ]
});

/// Validate and sanitize a file path
pub fn validate_path(path: &str) -> Result<PathBuf> {
    let path_buf = PathBuf::from(path);

    // Check for path traversal attempts in original path
    let path_str = path_buf.to_string_lossy();
    if path_str.contains("..") {
        warn!(path = %path, "Path traversal attempt detected");
        return Err(Error::PermissionDenied(
            "Path traversal (..) is not allowed".to_string(),
        ));
    }

    // Check original path against blocked directories first
    for blocked in BLOCKED_DIRECTORIES.iter() {
        if path_str.starts_with(blocked) {
            warn!(path = %path, blocked = %blocked, "Access to blocked directory");
            return Err(Error::PermissionDenied(format!(
                "Access to '{}' is restricted",
                blocked
            )));
        }
    }

    // SECURITY: Canonicalize to resolve symlinks and prevent symlink attacks
    let canonical = if path_buf.exists() {
        path_buf.canonicalize().map_err(|e| {
            warn!(path = %path, error = %e, "Failed to canonicalize path");
            Error::PermissionDenied(format!("Cannot resolve path '{}': {}", path, e))
        })?
    } else if let Some(parent) = path_buf.parent() {
        if parent.as_os_str().is_empty() || !parent.exists() {
            path_buf.clone()
        } else {
            let canonical_parent = parent.canonicalize().map_err(|e| {
                warn!(path = %path, error = %e, "Failed to canonicalize parent directory");
                Error::PermissionDenied(format!(
                    "Cannot resolve parent directory of '{}': {}",
                    path, e
                ))
            })?;
            if let Some(filename) = path_buf.file_name() {
                canonical_parent.join(filename)
            } else {
                canonical_parent
            }
        }
    } else {
        path_buf.clone()
    };

    let canonical_str = canonical.to_string_lossy();
    for blocked in BLOCKED_DIRECTORIES.iter() {
        if canonical_str.starts_with(blocked) {
            warn!(
                original_path = %path,
                resolved_path = %canonical_str,
                blocked = %blocked,
                "Symlink attack blocked: path resolves to restricted directory"
            );
            return Err(Error::PermissionDenied(format!(
                "Access denied: path resolves to restricted area '{}' (potential symlink attack)",
                blocked
            )));
        }
    }

    if canonical_str.contains("..") {
        warn!(path = %path, canonical = %canonical_str, "Path traversal in canonicalized path");
        return Err(Error::PermissionDenied(
            "Path traversal detected after canonicalization".to_string(),
        ));
    }

    debug!(original = %path, canonical = %canonical_str, "Path validated and canonicalized");
    Ok(canonical)
}

/// Check if a file is sensitive based on its name
pub fn is_sensitive_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    SENSITIVE_FILE_PATTERNS
        .iter()
        .any(|pattern| file_name.contains(pattern))
}

pub fn content_appears_sensitive(content: &str) -> bool {
    let lower = content.to_lowercase();
    let patterns = [
        "api_key=",
        "apikey=",
        "api-key=",
        "secret=",
        "password=",
        "passwd=",
        "token=",
        "bearer ",
        "authorization:",
        "aws_secret",
        "private_key",
        "-----begin",
        "-----begin rsa",
        "-----begin openssh",
    ];

    patterns.iter().any(|p| lower.contains(p))
}
