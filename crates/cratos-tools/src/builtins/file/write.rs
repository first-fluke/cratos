use super::security;
use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::time::Instant;
use tracing::{debug, warn};

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
        let file_path = security::validate_path(path)?;

        // SECURITY: Block writing to sensitive file locations
        if security::is_sensitive_file(&file_path) {
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
        if security::content_appears_sensitive(content) {
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
