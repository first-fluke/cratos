//! File tools - Read, write, and list files

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::path::PathBuf;
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tracing::debug;

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

        let max_bytes = input
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(1_048_576) as usize;

        debug!(path = %path, max_bytes = %max_bytes, "Reading file");

        // Read file
        let file_path = PathBuf::from(path);
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

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'content' parameter".to_string()))?;

        let append = input
            .get("append")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let create_dirs = input
            .get("create_dirs")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        debug!(path = %path, append = %append, "Writing file");

        let file_path = PathBuf::from(path);

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

        let max_entries = input
            .get("max_entries")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;

        debug!(path = %path, "Listing directory");

        let dir_path = PathBuf::from(path);
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
}
