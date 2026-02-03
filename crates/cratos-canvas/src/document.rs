//! Canvas Document Types
//!
//! This module defines the document structure for the Live Canvas system.
//! A document consists of blocks that can contain markdown, code, diagrams, charts, or images.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A canvas document containing multiple blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasDocument {
    /// Unique identifier
    pub id: Uuid,

    /// Document title
    pub title: String,

    /// Ordered list of blocks
    pub blocks: Vec<CanvasBlock>,

    /// When the document was created
    pub created_at: DateTime<Utc>,

    /// When the document was last modified
    pub updated_at: DateTime<Utc>,

    /// Document metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl CanvasDocument {
    /// Create a new empty document
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            blocks: Vec::new(),
            created_at: now,
            updated_at: now,
            metadata: serde_json::json!({}),
        }
    }

    /// Create with a specific ID
    #[must_use]
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Add a block to the document
    pub fn add_block(&mut self, block: CanvasBlock) {
        self.blocks.push(block);
        self.updated_at = Utc::now();
    }

    /// Insert a block at a specific position
    pub fn insert_block(&mut self, index: usize, block: CanvasBlock) {
        let index = index.min(self.blocks.len());
        self.blocks.insert(index, block);
        self.updated_at = Utc::now();
    }

    /// Remove a block by ID
    pub fn remove_block(&mut self, block_id: Uuid) -> Option<CanvasBlock> {
        if let Some(pos) = self.blocks.iter().position(|b| b.id() == block_id) {
            self.updated_at = Utc::now();
            Some(self.blocks.remove(pos))
        } else {
            None
        }
    }

    /// Get a block by ID
    #[must_use]
    pub fn get_block(&self, block_id: Uuid) -> Option<&CanvasBlock> {
        self.blocks.iter().find(|b| b.id() == block_id)
    }

    /// Get a mutable reference to a block by ID
    pub fn get_block_mut(&mut self, block_id: Uuid) -> Option<&mut CanvasBlock> {
        self.blocks.iter_mut().find(|b| b.id() == block_id)
    }

    /// Update a block's content
    pub fn update_block(&mut self, block_id: Uuid, content: String) -> bool {
        if let Some(block) = self.get_block_mut(block_id) {
            block.set_content(content);
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Get the number of blocks
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }
}

/// Block types for canvas documents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CanvasBlock {
    /// Markdown text block
    Markdown {
        /// Unique block ID
        id: Uuid,
        /// Markdown content
        content: String,
        /// When the block was created
        created_at: DateTime<Utc>,
        /// When the block was last modified
        updated_at: DateTime<Utc>,
    },

    /// Code block with syntax highlighting
    Code {
        /// Unique block ID
        id: Uuid,
        /// Programming language
        language: String,
        /// Code content
        content: String,
        /// Whether the code is executable
        executable: bool,
        /// When the block was created
        created_at: DateTime<Utc>,
        /// When the block was last modified
        updated_at: DateTime<Utc>,
    },

    /// Diagram block (rendered via external service like Kroki)
    Diagram {
        /// Unique block ID
        id: Uuid,
        /// Diagram type (mermaid, plantuml, graphviz, etc.)
        diagram_type: DiagramType,
        /// Diagram source
        source: String,
        /// When the block was created
        created_at: DateTime<Utc>,
        /// When the block was last modified
        updated_at: DateTime<Utc>,
    },

    /// Image block
    Image {
        /// Unique block ID
        id: Uuid,
        /// Image URL or base64 data
        url: String,
        /// Alt text
        alt: String,
        /// When the block was created
        created_at: DateTime<Utc>,
        /// When the block was last modified
        updated_at: DateTime<Utc>,
    },

    /// Chart block (rendered via plotters)
    Chart {
        /// Unique block ID
        id: Uuid,
        /// Chart type
        chart_type: ChartType,
        /// Chart data
        data: serde_json::Value,
        /// When the block was created
        created_at: DateTime<Utc>,
        /// When the block was last modified
        updated_at: DateTime<Utc>,
    },
}

impl CanvasBlock {
    /// Create a new markdown block
    #[must_use]
    pub fn markdown(content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::Markdown {
            id: Uuid::new_v4(),
            content: content.into(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new code block
    #[must_use]
    pub fn code(language: impl Into<String>, content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::Code {
            id: Uuid::new_v4(),
            language: language.into(),
            content: content.into(),
            executable: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create an executable code block
    #[must_use]
    pub fn executable_code(language: impl Into<String>, content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::Code {
            id: Uuid::new_v4(),
            language: language.into(),
            content: content.into(),
            executable: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new diagram block
    #[must_use]
    pub fn diagram(diagram_type: DiagramType, source: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::Diagram {
            id: Uuid::new_v4(),
            diagram_type,
            source: source.into(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new image block
    #[must_use]
    pub fn image(url: impl Into<String>, alt: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::Image {
            id: Uuid::new_v4(),
            url: url.into(),
            alt: alt.into(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new chart block
    #[must_use]
    pub fn chart(chart_type: ChartType, data: serde_json::Value) -> Self {
        let now = Utc::now();
        Self::Chart {
            id: Uuid::new_v4(),
            chart_type,
            data,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get the block ID
    #[must_use]
    pub fn id(&self) -> Uuid {
        match self {
            Self::Markdown { id, .. }
            | Self::Code { id, .. }
            | Self::Diagram { id, .. }
            | Self::Image { id, .. }
            | Self::Chart { id, .. } => *id,
        }
    }

    /// Get the block type as a string
    #[must_use]
    pub fn block_type(&self) -> &'static str {
        match self {
            Self::Markdown { .. } => "markdown",
            Self::Code { .. } => "code",
            Self::Diagram { .. } => "diagram",
            Self::Image { .. } => "image",
            Self::Chart { .. } => "chart",
        }
    }

    /// Get the content of the block
    #[must_use]
    pub fn content(&self) -> &str {
        match self {
            Self::Markdown { content, .. } | Self::Code { content, .. } => content,
            Self::Diagram { source, .. } => source,
            Self::Image { url, .. } => url,
            Self::Chart { .. } => "",
        }
    }

    /// Set the content of the block
    pub fn set_content(&mut self, new_content: String) {
        let now = Utc::now();
        match self {
            Self::Markdown {
                content,
                updated_at,
                ..
            } => {
                *content = new_content;
                *updated_at = now;
            }
            Self::Code {
                content,
                updated_at,
                ..
            } => {
                *content = new_content;
                *updated_at = now;
            }
            Self::Diagram {
                source, updated_at, ..
            } => {
                *source = new_content;
                *updated_at = now;
            }
            Self::Image {
                url, updated_at, ..
            } => {
                *url = new_content;
                *updated_at = now;
            }
            Self::Chart { updated_at, .. } => {
                *updated_at = now;
            }
        }
    }

    /// Get the creation timestamp
    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        match self {
            Self::Markdown { created_at, .. }
            | Self::Code { created_at, .. }
            | Self::Diagram { created_at, .. }
            | Self::Image { created_at, .. }
            | Self::Chart { created_at, .. } => *created_at,
        }
    }

    /// Get the last update timestamp
    #[must_use]
    pub fn updated_at(&self) -> DateTime<Utc> {
        match self {
            Self::Markdown { updated_at, .. }
            | Self::Code { updated_at, .. }
            | Self::Diagram { updated_at, .. }
            | Self::Image { updated_at, .. }
            | Self::Chart { updated_at, .. } => *updated_at,
        }
    }
}

/// Supported diagram types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagramType {
    /// Mermaid diagrams
    Mermaid,
    /// PlantUML diagrams
    Plantuml,
    /// Graphviz/DOT diagrams
    Graphviz,
    /// D2 diagrams
    D2,
    /// Sequence diagrams
    Sequence,
    /// Flowcharts
    Flowchart,
}

impl DiagramType {
    /// Get the Kroki endpoint for this diagram type
    #[must_use]
    pub fn kroki_type(&self) -> &'static str {
        match self {
            Self::Mermaid => "mermaid",
            Self::Plantuml => "plantuml",
            Self::Graphviz => "graphviz",
            Self::D2 => "d2",
            Self::Sequence => "seqdiag",
            Self::Flowchart => "mermaid",
        }
    }
}

/// Supported chart types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    /// Line chart
    Line,
    /// Bar chart
    Bar,
    /// Pie chart
    Pie,
    /// Scatter plot
    Scatter,
    /// Area chart
    Area,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_document_creation() {
        let doc = CanvasDocument::new("Test Document");
        assert_eq!(doc.title, "Test Document");
        assert!(doc.blocks.is_empty());
    }

    #[test]
    fn test_canvas_document_add_block() {
        let mut doc = CanvasDocument::new("Test");
        doc.add_block(CanvasBlock::markdown("Hello, world!"));
        assert_eq!(doc.block_count(), 1);
    }

    #[test]
    fn test_canvas_block_markdown() {
        let block = CanvasBlock::markdown("# Hello");
        assert_eq!(block.block_type(), "markdown");
        assert_eq!(block.content(), "# Hello");
    }

    #[test]
    fn test_canvas_block_code() {
        let block = CanvasBlock::code("rust", "fn main() {}");
        assert_eq!(block.block_type(), "code");
        assert_eq!(block.content(), "fn main() {}");
    }

    #[test]
    fn test_canvas_block_diagram() {
        let block = CanvasBlock::diagram(DiagramType::Mermaid, "graph TD; A-->B;");
        assert_eq!(block.block_type(), "diagram");
        match block {
            CanvasBlock::Diagram { diagram_type, .. } => {
                assert_eq!(diagram_type, DiagramType::Mermaid);
            }
            _ => panic!("Expected Diagram block"),
        }
    }

    #[test]
    fn test_canvas_document_remove_block() {
        let mut doc = CanvasDocument::new("Test");
        let block = CanvasBlock::markdown("To be removed");
        let block_id = block.id();
        doc.add_block(block);

        assert_eq!(doc.block_count(), 1);
        let removed = doc.remove_block(block_id);
        assert!(removed.is_some());
        assert_eq!(doc.block_count(), 0);
    }

    #[test]
    fn test_canvas_block_update_content() {
        let mut doc = CanvasDocument::new("Test");
        let block = CanvasBlock::markdown("Original");
        let block_id = block.id();
        doc.add_block(block);

        assert!(doc.update_block(block_id, "Updated".to_string()));
        assert_eq!(doc.get_block(block_id).unwrap().content(), "Updated");
    }

    #[test]
    fn test_canvas_block_serialization() {
        let block = CanvasBlock::code("python", "print('hello')");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"code\""));
        assert!(json.contains("\"language\":\"python\""));

        let parsed: CanvasBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content(), "print('hello')");
    }
}
