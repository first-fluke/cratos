use super::security;
use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Default maximum entries for directory listing
const DEFAULT_MAX_ENTRIES: u64 = 1000;

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
        let dir_path = security::validate_path(path)?;

        let max_entries = input
            .get("max_entries")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_MAX_ENTRIES) as usize;

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
