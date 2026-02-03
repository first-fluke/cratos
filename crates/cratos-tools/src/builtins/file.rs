//! File tools - Read, write, and list files

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;
use tokio::io::AsyncReadExt;
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
///
/// # Security
///
/// This function protects against:
/// 1. Path traversal attacks using ".."
/// 2. Symlink attacks by canonicalizing paths
/// 3. Access to sensitive system directories
fn validate_path(path: &str) -> Result<PathBuf> {
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
    // For existing files/dirs, use canonicalize() to get the real path
    // For non-existing files, canonicalize the parent directory
    let canonical = if path_buf.exists() {
        path_buf.canonicalize().map_err(|e| {
            warn!(path = %path, error = %e, "Failed to canonicalize path");
            Error::PermissionDenied(format!(
                "Cannot resolve path '{}': {}",
                path, e
            ))
        })?
    } else {
        // For new files, canonicalize the parent directory
        if let Some(parent) = path_buf.parent() {
            if parent.as_os_str().is_empty() || !parent.exists() {
                // Parent doesn't exist, use original path (directory will be created)
                path_buf.clone()
            } else {
                let canonical_parent = parent.canonicalize().map_err(|e| {
                    warn!(path = %path, error = %e, "Failed to canonicalize parent directory");
                    Error::PermissionDenied(format!(
                        "Cannot resolve parent directory of '{}': {}",
                        path, e
                    ))
                })?;
                // Append the filename to the canonicalized parent
                if let Some(filename) = path_buf.file_name() {
                    canonical_parent.join(filename)
                } else {
                    canonical_parent
                }
            }
        } else {
            path_buf.clone()
        }
    };

    // SECURITY: Check canonicalized path against blocked directories
    // This catches symlink attacks where a link points to restricted areas
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

    // Also check if canonicalized path contains ".." (shouldn't happen but defense in depth)
    if canonical_str.contains("..") {
        warn!(path = %path, canonical = %canonical_str, "Path traversal in canonicalized path");
        return Err(Error::PermissionDenied(
            "Path traversal detected after canonicalization".to_string(),
        ));
    }

    debug!(original = %path, canonical = %canonical_str, "Path validated and canonicalized");
    Ok(canonical)
}

/// Check if a file appears to be sensitive
fn is_sensitive_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    SENSITIVE_FILE_PATTERNS
        .iter()
        .any(|pattern| file_name.contains(pattern))
}

/// Check if content appears to contain secrets
fn content_appears_sensitive(content: &str) -> bool {
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

// ============================================================================
// File Read Tool
// ============================================================================

/// Tool for reading file contents
pub struct FileReadTool {
    definition: ToolDefinition,
}

impl FileReadTool {
    /// Create a new file read tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new("file_read", "Read the contents of a file")
            .with_category(ToolCategory::File)
            .with_risk_level(RiskLevel::Low)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    },
                    "encoding": {
                        "type": "string",
                        "description": "File encoding (default: utf-8)",
                        "default": "utf-8"
                    },
                    "max_bytes": {
                        "type": "integer",
                        "description": "Maximum bytes to read (default: 1MB)",
                        "default": 1048576
                    }
                },
                "required": ["path"]
            }));

        Self { definition }
    }
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for FileReadTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'path' parameter".to_string()))?;

        // SECURITY: Validate path
        let file_path = validate_path(path)?;

        // SECURITY: Warn about sensitive files
        if is_sensitive_file(&file_path) {
            warn!(path = %path, "Attempt to read potentially sensitive file");
            return Err(Error::PermissionDenied(format!(
                "Reading '{}' is restricted - file appears to contain sensitive data",
                file_path.file_name().unwrap_or_default().to_string_lossy()
            )));
        }

        let max_bytes = input
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(1_048_576) as usize;

        debug!(path = %path, max_bytes = %max_bytes, "Reading file");

        // Read file
        let file = tokio::fs::File::open(&file_path).await.map_err(Error::Io)?;

        let mut contents = Vec::new();
        let mut take = file.take(max_bytes as u64);
        take.read_to_end(&mut contents).await.map_err(Error::Io)?;

        let content = String::from_utf8_lossy(&contents).to_string();
        let truncated = contents.len() >= max_bytes;

        let duration = start.elapsed().as_millis() as u64;

        Ok(ToolResult::success(
            serde_json::json!({
                "content": content,
                "path": path,
                "size": contents.len(),
                "truncated": truncated
            }),
            duration,
        ))
    }
}

// ============================================================================
// File Write Tool
// ============================================================================

/// Tool for writing file contents
pub struct FileWriteTool {
    definition: ToolDefinition,
}

impl FileWriteTool {
    /// Create a new file write tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new("file_write", "Write content to a file")
            .with_category(ToolCategory::File)
            .with_risk_level(RiskLevel::Medium)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    },
                    "append": {
                        "type": "boolean",
                        "description": "Append to file instead of overwriting",
                        "default": false
                    },
                    "create_dirs": {
                        "type": "boolean",
                        "description": "Create parent directories if they don't exist",
                        "default": false
                    }
                },
                "required": ["path", "content"]
            }));

        Self { definition }
    }
}

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for FileWriteTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'path' parameter".to_string()))?;

        // SECURITY: Validate path
        let file_path = validate_path(path)?;

        // SECURITY: Block writing to sensitive file locations
        if is_sensitive_file(&file_path) {
            warn!(path = %path, "Attempt to write to potentially sensitive file");
            return Err(Error::PermissionDenied(format!(
                "Writing to '{}' is restricted - file appears to be sensitive",
                file_path.file_name().unwrap_or_default().to_string_lossy()
            )));
        }

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'content' parameter".to_string()))?;

        // SECURITY: Check if content contains secrets
        if content_appears_sensitive(content) {
            warn!(path = %path, "Attempt to write content containing potential secrets");
            return Err(Error::PermissionDenied(
                "Content appears to contain sensitive data (API keys, passwords, tokens). \
                 Writing secrets to files is blocked for security."
                    .to_string(),
            ));
        }

        let append = input
            .get("append")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let create_dirs = input
            .get("create_dirs")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        debug!(path = %path, append = %append, "Writing file");


        // Create parent directories if requested
        if create_dirs {
            if let Some(parent) = file_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(Error::Io)?;
            }
        }

        // Write file
        if append {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .await
                .map_err(Error::Io)?;
            file.write_all(content.as_bytes())
                .await
                .map_err(Error::Io)?;
        } else {
            tokio::fs::write(&file_path, content)
                .await
                .map_err(Error::Io)?;
        }

        let duration = start.elapsed().as_millis() as u64;

        Ok(ToolResult::success(
            serde_json::json!({
                "path": path,
                "bytes_written": content.len(),
                "append": append
            }),
            duration,
        ))
    }
}

// ============================================================================
// File List Tool
// ============================================================================

/// Tool for listing directory contents
pub struct FileListTool {
    definition: ToolDefinition,
}

impl FileListTool {
    /// Create a new file list tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new("file_list", "List contents of a directory")
            .with_category(ToolCategory::File)
            .with_risk_level(RiskLevel::Low)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the directory to list"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "List recursively",
                        "default": false
                    },
                    "max_entries": {
                        "type": "integer",
                        "description": "Maximum entries to return",
                        "default": 1000
                    }
                },
                "required": ["path"]
            }));

        Self { definition }
    }
}

impl Default for FileListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for FileListTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'path' parameter".to_string()))?;

        // SECURITY: Validate path
        let dir_path = validate_path(path)?;

        let max_entries = input
            .get("max_entries")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;

        debug!(path = %path, "Listing directory");
        let mut entries = Vec::new();

        let mut dir = tokio::fs::read_dir(&dir_path).await.map_err(Error::Io)?;

        while let Some(entry) = dir.next_entry().await.map_err(Error::Io)? {
            if entries.len() >= max_entries {
                break;
            }

            let metadata = entry.metadata().await.ok();
            let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

            entries.push(serde_json::json!({
                "name": entry.file_name().to_string_lossy(),
                "path": entry.path().to_string_lossy(),
                "is_dir": is_dir,
                "size": size
            }));
        }

        let duration = start.elapsed().as_millis() as u64;

        Ok(ToolResult::success(
            serde_json::json!({
                "path": path,
                "entries": entries,
                "count": entries.len(),
                "truncated": entries.len() >= max_entries
            }),
            duration,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Tool;

    #[test]
    fn test_file_read_definition() {
        let tool = FileReadTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "file_read");
        assert_eq!(def.risk_level, RiskLevel::Low);
        assert_eq!(def.category, ToolCategory::File);
    }

    #[tokio::test]
    async fn test_file_read_missing_path() {
        let tool = FileReadTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_path_traversal_blocked() {
        assert!(validate_path("../../../etc/passwd").is_err());
        assert!(validate_path("/etc/passwd").is_err());
        assert!(validate_path("/root/.ssh/id_rsa").is_err());
        assert!(validate_path("/var/log/syslog").is_err());

        // Safe paths should pass (note: canonicalize may fail for non-existent paths)
        // /tmp typically exists, so this should work
        #[cfg(unix)]
        assert!(validate_path("/tmp").is_ok());
    }

    #[test]
    fn test_symlink_attack_prevention() {
        // The validate_path function now uses canonicalize() to resolve symlinks
        // This ensures that even if a symlink points to /etc, it will be blocked

        // Direct access to blocked directories should fail
        assert!(validate_path("/etc").is_err());
        assert!(validate_path("/root").is_err());
        assert!(validate_path("/var/log").is_err());
        assert!(validate_path("/boot").is_err());
        assert!(validate_path("/proc").is_err());
        assert!(validate_path("/sys").is_err());

        // Path traversal should fail
        assert!(validate_path("/tmp/../etc/passwd").is_err());
    }

    #[test]
    fn test_sensitive_file_detection() {
        assert!(is_sensitive_file(Path::new(".env")));
        assert!(is_sensitive_file(Path::new(".env.production")));
        assert!(is_sensitive_file(Path::new("credentials.json")));
        assert!(is_sensitive_file(Path::new("id_rsa")));
        assert!(is_sensitive_file(Path::new("api_key.txt")));

        // Non-sensitive files should pass
        assert!(!is_sensitive_file(Path::new("main.rs")));
        assert!(!is_sensitive_file(Path::new("config.toml")));
        assert!(!is_sensitive_file(Path::new("README.md")));
    }

    #[test]
    fn test_sensitive_content_detection() {
        assert!(content_appears_sensitive("API_KEY=sk-1234567890"));
        assert!(content_appears_sensitive("password=secret123"));
        assert!(content_appears_sensitive("Bearer eyJhbGciOiJIUzI1NiJ9"));
        assert!(content_appears_sensitive("-----BEGIN RSA PRIVATE KEY-----"));
        assert!(content_appears_sensitive("aws_secret_access_key=AKIAIOSFODNN7EXAMPLE"));

        // Normal content should pass
        assert!(!content_appears_sensitive("Hello, world!"));
        assert!(!content_appears_sensitive("fn main() { println!(\"Hello\"); }"));
        assert!(!content_appears_sensitive("# Configuration\nport = 8080"));
    }

    #[tokio::test]
    async fn test_file_read_blocks_sensitive() {
        let tool = FileReadTool::new();

        let result = tool
            .execute(serde_json::json!({
                "path": "/home/user/.env"
            }))
            .await;
        assert!(result.is_err());

        let result = tool
            .execute(serde_json::json!({
                "path": "/etc/passwd"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_write_blocks_secrets() {
        let tool = FileWriteTool::new();

        let result = tool
            .execute(serde_json::json!({
                "path": "/tmp/test.txt",
                "content": "API_KEY=sk-secret123"
            }))
            .await;
        assert!(result.is_err());
    }
}
