//! Content Rendering
//!
//! This module provides rendering utilities for canvas blocks.
//! Converts markdown to HTML, highlights code, and generates diagram URLs.

use pulldown_cmark::{html, Options, Parser};
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

use crate::document::{CanvasBlock, DiagramType};

/// Renderer for canvas content
pub struct ContentRenderer {
    /// Syntax highlighting syntax set
    syntax_set: SyntaxSet,
    /// Syntax highlighting theme set
    theme_set: ThemeSet,
    /// Default theme name
    theme_name: String,
    /// Kroki server URL (for diagrams)
    kroki_url: String,
}

impl ContentRenderer {
    /// Create a new renderer with default settings
    #[must_use]
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            theme_name: "base16-ocean.dark".to_string(),
            kroki_url: "https://kroki.io".to_string(),
        }
    }

    /// Set the syntax highlighting theme
    #[must_use]
    pub fn with_theme(mut self, theme_name: impl Into<String>) -> Self {
        self.theme_name = theme_name.into();
        self
    }

    /// Set the Kroki server URL
    #[must_use]
    pub fn with_kroki_url(mut self, url: impl Into<String>) -> Self {
        self.kroki_url = url.into();
        self
    }

    /// Render markdown to HTML
    #[must_use]
    pub fn render_markdown(&self, markdown: &str) -> String {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(markdown, options);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        html_output
    }

    /// Render code with syntax highlighting
    #[must_use]
    pub fn render_code(&self, code: &str, language: &str) -> String {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(language)
            .or_else(|| self.syntax_set.find_syntax_by_name(language))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = self
            .theme_set
            .themes
            .get(&self.theme_name)
            .unwrap_or_else(|| self.theme_set.themes.values().next().unwrap());

        highlighted_html_for_string(code, &self.syntax_set, syntax, theme)
            .unwrap_or_else(|_| format!("<pre><code>{}</code></pre>", html_escape(code)))
    }

    /// Generate a Kroki URL for a diagram
    #[must_use]
    pub fn diagram_url(&self, diagram_type: DiagramType, source: &str) -> String {
        let encoded = encode_diagram(source);
        format!(
            "{}/{}/svg/{}",
            self.kroki_url,
            diagram_type.kroki_type(),
            encoded
        )
    }

    /// Render a canvas block to HTML
    #[must_use]
    pub fn render_block(&self, block: &CanvasBlock) -> RenderedBlock {
        match block {
            CanvasBlock::Markdown { content, .. } => RenderedBlock {
                html: self.render_markdown(content),
                block_type: "markdown".to_string(),
                language: None,
                source_url: None,
            },
            CanvasBlock::Code {
                content, language, ..
            } => RenderedBlock {
                html: self.render_code(content, language),
                block_type: "code".to_string(),
                language: Some(language.clone()),
                source_url: None,
            },
            CanvasBlock::Diagram {
                diagram_type,
                source,
                ..
            } => RenderedBlock {
                html: format!(
                    r#"<img src="{}" alt="Diagram" class="diagram" loading="lazy" />"#,
                    self.diagram_url(*diagram_type, source)
                ),
                block_type: "diagram".to_string(),
                language: None,
                source_url: Some(self.diagram_url(*diagram_type, source)),
            },
            CanvasBlock::Image { url, alt, .. } => RenderedBlock {
                html: format!(
                    r#"<img src="{}" alt="{}" class="image" loading="lazy" />"#,
                    html_escape(url),
                    html_escape(alt)
                ),
                block_type: "image".to_string(),
                language: None,
                source_url: Some(url.clone()),
            },
            CanvasBlock::Chart { chart_type, .. } => RenderedBlock {
                html: format!(
                    r#"<div class="chart" data-type="{}">Chart rendering requires JavaScript</div>"#,
                    chart_type.as_ref()
                ),
                block_type: "chart".to_string(),
                language: None,
                source_url: None,
            },
        }
    }
}

impl Default for ContentRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Rendered block output
#[derive(Debug, Clone)]
pub struct RenderedBlock {
    /// Rendered HTML content
    pub html: String,
    /// Block type
    pub block_type: String,
    /// Programming language (for code blocks)
    pub language: Option<String>,
    /// Source URL (for images, diagrams)
    pub source_url: Option<String>,
}

/// Encode diagram source for Kroki URL
fn encode_diagram(source: &str) -> String {
    use base64::Engine;
    use std::io::Write;

    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(source.as_bytes()).unwrap_or_default();
    let compressed = encoder.finish().unwrap_or_default();

    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compressed)
}

/// Escape HTML special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Chart type for display
impl AsRef<str> for crate::document::ChartType {
    fn as_ref(&self) -> &str {
        match self {
            Self::Line => "line",
            Self::Bar => "bar",
            Self::Pie => "pie",
            Self::Scatter => "scatter",
            Self::Area => "area",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_markdown() {
        let renderer = ContentRenderer::new();
        let html = renderer.render_markdown("# Hello\n\nWorld");
        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello"));
        assert!(html.contains("World"));
    }

    #[test]
    fn test_render_markdown_with_table() {
        let renderer = ContentRenderer::new();
        let markdown = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = renderer.render_markdown(markdown);
        assert!(html.contains("<table>"));
    }

    #[test]
    fn test_render_code() {
        let renderer = ContentRenderer::new();
        let html = renderer.render_code("fn main() {}", "rust");
        assert!(html.contains("fn"));
        assert!(html.contains("main"));
    }

    #[test]
    fn test_render_code_unknown_language() {
        let renderer = ContentRenderer::new();
        let html = renderer.render_code("some code", "unknown_lang_xyz");
        assert!(html.contains("some code"));
    }

    #[test]
    fn test_diagram_url() {
        let renderer = ContentRenderer::new();
        let url = renderer.diagram_url(DiagramType::Mermaid, "graph TD; A-->B;");
        assert!(url.starts_with("https://kroki.io/mermaid/svg/"));
    }

    #[test]
    fn test_render_markdown_block() {
        let renderer = ContentRenderer::new();
        let block = CanvasBlock::markdown("**Bold**");
        let rendered = renderer.render_block(&block);

        assert_eq!(rendered.block_type, "markdown");
        assert!(rendered.html.contains("<strong>"));
    }

    #[test]
    fn test_render_code_block() {
        let renderer = ContentRenderer::new();
        let block = CanvasBlock::code("python", "print('hello')");
        let rendered = renderer.render_block(&block);

        assert_eq!(rendered.block_type, "code");
        assert_eq!(rendered.language, Some("python".to_string()));
    }

    #[test]
    fn test_render_image_block() {
        let renderer = ContentRenderer::new();
        let block = CanvasBlock::image("https://example.com/img.png", "Example");
        let rendered = renderer.render_block(&block);

        assert_eq!(rendered.block_type, "image");
        assert!(rendered.html.contains("img.png"));
        assert!(rendered.html.contains("Example"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }
}
