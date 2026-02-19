use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::path::Path;
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tracing::{debug, warn};
use super::security;

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
                        "description": "File encoding: 'utf-8' (default) for text, 'binary' for base64-encoded binary",
                        "enum": ["utf-8", "binary"],
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

    fn infer_mime_type(path: &Path) -> String {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "pdf" => "application/pdf",
            "zip" => "application/zip",
            "tar" => "application/x-tar",
            "gz" => "application/gzip",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            _ => "application/octet-stream",
        }
        .to_string()
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
        let file_path = security::validate_path(path)?;

        // SECURITY: Warn about sensitive files
        if security::is_sensitive_file(&file_path) {
            warn!(path = %path, "Attempt to read potentially sensitive file");
            return Err(Error::PermissionDenied(format!(
                "Reading '{}' is restricted - file appears to contain sensitive data",
                file_path.file_name().unwrap_or_default().to_string_lossy()
            )));
        }

        let encoding = input
            .get("encoding")
            .and_then(|v| v.as_str())
            .unwrap_or("utf-8");

        let max_bytes = input
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(1_048_576) as usize;

        debug!(path = %path, encoding = %encoding, max_bytes = %max_bytes, "Reading file");

        // Read file
        let file = tokio::fs::File::open(&file_path).await.map_err(Error::Io)?;

        let mut contents = Vec::new();
        let mut take = file.take(max_bytes as u64);
        take.read_to_end(&mut contents).await.map_err(Error::Io)?;

        let truncated = contents.len() >= max_bytes;
        let duration = start.elapsed().as_millis() as u64;

        // Handle binary mode
        if encoding == "binary" {
            let mime_type = Self::infer_mime_type(&file_path);
            let base64_data = BASE64.encode(&contents);

            return Ok(ToolResult::success(
                serde_json::json!({
                    "encoding": "binary",
                    "mime_type": mime_type,
                    "data": base64_data,
                    "path": path,
                    "size": contents.len(),
                    "truncated": truncated
                }),
                duration,
            ));
        }

        // Default: UTF-8 text mode
        let content = String::from_utf8_lossy(&contents).to_string();

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
