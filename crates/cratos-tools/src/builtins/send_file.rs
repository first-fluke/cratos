//! Send File Tool - Prepares files for sending through channels
//!
//! This tool reads a file and returns it as an artifact that can be
//! sent through messaging channels (Telegram, Slack, Discord, etc.).

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::time::Instant;
use tracing::{debug, warn};

// Re-use security functions from file.rs
use super::file::{is_sensitive_file, validate_path};

/// Maximum file size for sending (50 MB)
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;

/// Blocked MIME types (executables)
const BLOCKED_MIME_TYPES: &[&str] = &[
    "application/x-executable",
    "application/x-msdos-program",
    "application/x-msdownload",
    "application/x-sh",
    "application/x-shellscript",
];

/// Tool for preparing files to send through channels
pub struct SendFileTool {
    definition: ToolDefinition,
}

impl SendFileTool {
    /// Create a new send file tool
    #[must_use]
    pub fn new() -> Self {
        let definition = ToolDefinition::new(
            "send_file",
            "Prepare a file for sending through messaging channels. Returns file data as an artifact.",
        )
        .with_category(ToolCategory::File)
        .with_risk_level(RiskLevel::Medium)
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to send"
                },
                "caption": {
                    "type": "string",
                    "description": "Optional caption/description for the file"
                }
            },
            "required": ["path"]
        }));

        Self { definition }
    }

    /// Infer MIME type from file extension
    fn infer_mime_type(path: &std::path::Path) -> String {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            // Images
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            "bmp" => "image/bmp",
            "tiff" | "tif" => "image/tiff",
            // Documents
            "pdf" => "application/pdf",
            "doc" => "application/msword",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "xls" => "application/vnd.ms-excel",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "ppt" => "application/vnd.ms-powerpoint",
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            // Text
            "txt" => "text/plain",
            "md" => "text/markdown",
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" => "text/javascript",
            "json" => "application/json",
            "xml" => "application/xml",
            "csv" => "text/csv",
            // Code
            "rs" => "text/x-rust",
            "py" => "text/x-python",
            "go" => "text/x-go",
            "java" => "text/x-java",
            "c" | "h" => "text/x-c",
            "cpp" | "hpp" => "text/x-c++",
            "ts" => "text/typescript",
            "toml" => "application/toml",
            "yaml" | "yml" => "application/x-yaml",
            // Archives
            "zip" => "application/zip",
            "tar" => "application/x-tar",
            "gz" => "application/gzip",
            "rar" => "application/vnd.rar",
            "7z" => "application/x-7z-compressed",
            // Audio
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "ogg" => "audio/ogg",
            "m4a" => "audio/mp4",
            "flac" => "audio/flac",
            // Video
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "avi" => "video/x-msvideo",
            "mkv" => "video/x-matroska",
            "mov" => "video/quicktime",
            // Default
            _ => "application/octet-stream",
        }
        .to_string()
    }
}

impl Default for SendFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for SendFileTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'path' parameter".to_string()))?;

        let caption = input.get("caption").and_then(|v| v.as_str());

        // SECURITY: Validate path (prevents path traversal and symlink attacks)
        let file_path = validate_path(path)?;

        // SECURITY: Block sensitive files
        if is_sensitive_file(&file_path) {
            warn!(path = %path, "Attempt to send potentially sensitive file");
            return Err(Error::PermissionDenied(format!(
                "Sending '{}' is restricted - file appears to contain sensitive data",
                file_path.file_name().unwrap_or_default().to_string_lossy()
            )));
        }

        // Check file exists
        if !file_path.exists() {
            return Err(Error::NotFound(format!("File not found: {}", path)));
        }

        // Check file size
        let metadata = tokio::fs::metadata(&file_path).await.map_err(Error::Io)?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(Error::InvalidInput(format!(
                "File too large: {} bytes (max {} MB)",
                metadata.len(),
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }

        // Infer MIME type
        let mime_type = Self::infer_mime_type(&file_path);

        // SECURITY: Block executable files
        if BLOCKED_MIME_TYPES.iter().any(|&m| mime_type == m) {
            warn!(path = %path, mime_type = %mime_type, "Blocked executable file");
            return Err(Error::PermissionDenied(
                "Sending executable files is blocked for security".to_string(),
            ));
        }

        // Read file content
        debug!(path = %path, size = %metadata.len(), "Reading file for sending");
        let data = tokio::fs::read(&file_path).await.map_err(Error::Io)?;

        // Encode as base64
        let base64_data = BASE64.encode(&data);

        // Get filename
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();

        let duration = start.elapsed().as_millis() as u64;

        // Return artifact info
        Ok(ToolResult::success(
            serde_json::json!({
                "artifact": {
                    "name": filename,
                    "mime_type": mime_type,
                    "data": base64_data
                },
                "path": path,
                "size": metadata.len(),
                "mime_type": mime_type,
                "caption": caption
            }),
            duration,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_send_file_definition() {
        let tool = SendFileTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "send_file");
        assert_eq!(def.risk_level, RiskLevel::Medium);
        assert_eq!(def.category, ToolCategory::File);
    }

    #[test]
    fn test_mime_type_inference() {
        assert_eq!(
            SendFileTool::infer_mime_type(Path::new("test.png")),
            "image/png"
        );
        assert_eq!(
            SendFileTool::infer_mime_type(Path::new("test.jpg")),
            "image/jpeg"
        );
        assert_eq!(
            SendFileTool::infer_mime_type(Path::new("test.pdf")),
            "application/pdf"
        );
        assert_eq!(
            SendFileTool::infer_mime_type(Path::new("test.rs")),
            "text/x-rust"
        );
        assert_eq!(
            SendFileTool::infer_mime_type(Path::new("test.unknown")),
            "application/octet-stream"
        );
    }

    #[tokio::test]
    async fn test_send_file_missing_path() {
        let tool = SendFileTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_file_blocks_sensitive() {
        let tool = SendFileTool::new();

        let result = tool
            .execute(serde_json::json!({
                "path": "/home/user/.env"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_file_blocks_system_paths() {
        let tool = SendFileTool::new();

        let result = tool
            .execute(serde_json::json!({
                "path": "/etc/passwd"
            }))
            .await;
        assert!(result.is_err());
    }
}
