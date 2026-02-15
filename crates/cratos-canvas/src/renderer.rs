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

        // Get the theme, falling back to first available or returning default HTML
        let Some(theme) = self
            .theme_set
            .themes
            .get(&self.theme_name)
            .or_else(|| self.theme_set.themes.values().next())
        else {
            return format!("<pre><code>{}</code></pre>", html_escape(code));
        };

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
            CanvasBlock::Chart {
                chart_type, data, ..
            } => RenderedBlock {
                html: render_chart_svg(*chart_type, data),
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

/// Render a chart to SVG
///
/// Supports basic chart types: Line, Bar, Pie, Scatter, Area.
/// Data format: `{"labels": ["A","B"], "datasets": [{"data": [10, 20], "color": "#4299e1"}]}`
fn render_chart_svg(chart_type: crate::document::ChartType, data: &serde_json::Value) -> String {
    use crate::document::ChartType;

    const WIDTH: f64 = 400.0;
    const HEIGHT: f64 = 250.0;
    const PADDING: f64 = 40.0;
    const CHART_WIDTH: f64 = WIDTH - PADDING * 2.0;
    const CHART_HEIGHT: f64 = HEIGHT - PADDING * 2.0;

    // Parse data
    let labels: Vec<String> = data["labels"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let datasets: Vec<(Vec<f64>, String)> = data["datasets"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|ds| {
                    let values: Vec<f64> = ds["data"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect())
                        .unwrap_or_default();
                    let color = ds["color"].as_str().unwrap_or("#4299e1").to_string();
                    (values, color)
                })
                .collect()
        })
        .unwrap_or_default();

    if datasets.is_empty() || datasets[0].0.is_empty() {
        return format!(
            r##"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">
                <rect width="100%" height="100%" fill="#1a1a2e"/>
                <text x="50%" y="50%" text-anchor="middle" fill="#888" font-size="14">No chart data</text>
            </svg>"##,
            WIDTH, HEIGHT
        );
    }

    let all_values: Vec<f64> = datasets
        .iter()
        .flat_map(|(v, _)| v.iter().copied())
        .collect();
    let max_val = all_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_val = all_values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min)
        .min(0.0);
    let range = (max_val - min_val).max(1.0);

    let mut svg = format!(
        r##"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">
            <rect width="100%" height="100%" fill="#1a1a2e"/>
            <g transform="translate({}, {})">"##,
        WIDTH, HEIGHT, PADDING, PADDING
    );

    // Y-axis
    svg.push_str(&format!(
        r##"<line x1="0" y1="0" x2="0" y2="{}" stroke="#444" stroke-width="1"/>"##,
        CHART_HEIGHT
    ));

    // X-axis
    svg.push_str(&format!(
        r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="#444" stroke-width="1"/>"##,
        CHART_HEIGHT, CHART_WIDTH, CHART_HEIGHT
    ));

    let n = datasets[0].0.len();

    match chart_type {
        ChartType::Bar => {
            let bar_width = CHART_WIDTH / n as f64 * 0.7;
            let gap = CHART_WIDTH / n as f64 * 0.15;

            for (di, (values, color)) in datasets.iter().enumerate() {
                for (i, &val) in values.iter().enumerate() {
                    let x = (i as f64 / n as f64) * CHART_WIDTH
                        + gap
                        + (di as f64 * bar_width / datasets.len() as f64);
                    let h = ((val - min_val) / range) * CHART_HEIGHT;
                    let y = CHART_HEIGHT - h;
                    svg.push_str(&format!(
                        r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{}" opacity="0.8"/>"#,
                        x, y, bar_width / datasets.len() as f64, h, color
                    ));
                }
            }
        }
        ChartType::Line | ChartType::Area => {
            for (values, color) in &datasets {
                let mut points = String::new();
                let mut area_points = format!("0,{:.1} ", CHART_HEIGHT);

                for (i, &val) in values.iter().enumerate() {
                    let x = (i as f64 / (n - 1).max(1) as f64) * CHART_WIDTH;
                    let y = CHART_HEIGHT - ((val - min_val) / range) * CHART_HEIGHT;
                    points.push_str(&format!("{:.1},{:.1} ", x, y));
                    area_points.push_str(&format!("{:.1},{:.1} ", x, y));
                }
                area_points.push_str(&format!("{:.1},{:.1}", CHART_WIDTH, CHART_HEIGHT));

                if chart_type == ChartType::Area {
                    svg.push_str(&format!(
                        r#"<polygon points="{}" fill="{}" opacity="0.3"/>"#,
                        area_points, color
                    ));
                }

                svg.push_str(&format!(
                    r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="2"/>"#,
                    points.trim(),
                    color
                ));

                // Data points
                for (i, &val) in values.iter().enumerate() {
                    let x = (i as f64 / (n - 1).max(1) as f64) * CHART_WIDTH;
                    let y = CHART_HEIGHT - ((val - min_val) / range) * CHART_HEIGHT;
                    svg.push_str(&format!(
                        r#"<circle cx="{:.1}" cy="{:.1}" r="4" fill="{}" />"#,
                        x, y, color
                    ));
                }
            }
        }
        ChartType::Scatter => {
            for (values, color) in &datasets {
                for (i, &val) in values.iter().enumerate() {
                    let x = (i as f64 / (n - 1).max(1) as f64) * CHART_WIDTH;
                    let y = CHART_HEIGHT - ((val - min_val) / range) * CHART_HEIGHT;
                    svg.push_str(&format!(
                        r#"<circle cx="{:.1}" cy="{:.1}" r="5" fill="{}" opacity="0.7"/>"#,
                        x, y, color
                    ));
                }
            }
        }
        ChartType::Pie => {
            let values = &datasets[0].0;
            let total: f64 = values.iter().sum();
            if total > 0.0 {
                let cx = CHART_WIDTH / 2.0;
                let cy = CHART_HEIGHT / 2.0;
                let r = CHART_HEIGHT.min(CHART_WIDTH) / 2.0 * 0.8;

                let colors = [
                    "#4299e1", "#48bb78", "#ed8936", "#e53e3e", "#805ad5", "#38b2ac",
                ];
                let mut start_angle = -std::f64::consts::FRAC_PI_2;

                for (i, &val) in values.iter().enumerate() {
                    let angle = (val / total) * 2.0 * std::f64::consts::PI;
                    let end_angle = start_angle + angle;

                    let x1 = cx + r * start_angle.cos();
                    let y1 = cy + r * start_angle.sin();
                    let x2 = cx + r * end_angle.cos();
                    let y2 = cy + r * end_angle.sin();

                    let large_arc = if angle > std::f64::consts::PI { 1 } else { 0 };
                    let color = colors[i % colors.len()];

                    svg.push_str(&format!(
                        r##"<path d="M{:.1},{:.1} L{:.1},{:.1} A{:.1},{:.1} 0 {} 1 {:.1},{:.1} Z" fill="{}" stroke="#1a1a2e" stroke-width="1"/>"##,
                        cx, cy, x1, y1, r, r, large_arc, x2, y2, color
                    ));

                    start_angle = end_angle;
                }
            }
        }
    }

    // X-axis labels
    for (i, label) in labels.iter().enumerate().take(n) {
        let x = match chart_type {
            ChartType::Bar => (i as f64 + 0.5) / n as f64 * CHART_WIDTH,
            _ => (i as f64 / (n - 1).max(1) as f64) * CHART_WIDTH,
        };
        svg.push_str(&format!(
            r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" fill="#888" font-size="10">{}</text>"##,
            x, CHART_HEIGHT + 15.0, html_escape(label)
        ));
    }

    svg.push_str("</g></svg>");
    svg
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

    #[test]
    fn test_render_chart_bar() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({
            "labels": ["A", "B", "C"],
            "datasets": [{"data": [10, 20, 30], "color": "#4299e1"}]
        });
        let block = CanvasBlock::chart(crate::document::ChartType::Bar, data);
        let rendered = renderer.render_block(&block);

        assert_eq!(rendered.block_type, "chart");
        assert!(rendered.html.contains("<svg"));
        assert!(rendered.html.contains("rect"));
    }

    #[test]
    fn test_render_chart_line() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({
            "labels": ["Jan", "Feb", "Mar"],
            "datasets": [{"data": [5, 15, 10], "color": "#48bb78"}]
        });
        let block = CanvasBlock::chart(crate::document::ChartType::Line, data);
        let rendered = renderer.render_block(&block);

        assert!(rendered.html.contains("<polyline"));
        assert!(rendered.html.contains("<circle"));
    }

    #[test]
    fn test_render_chart_pie() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({
            "labels": ["Red", "Blue", "Green"],
            "datasets": [{"data": [30, 50, 20]}]
        });
        let block = CanvasBlock::chart(crate::document::ChartType::Pie, data);
        let rendered = renderer.render_block(&block);

        assert!(rendered.html.contains("<path"));
    }

    #[test]
    fn test_render_chart_empty_data() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({});
        let block = CanvasBlock::chart(crate::document::ChartType::Bar, data);
        let rendered = renderer.render_block(&block);

        assert!(rendered.html.contains("No chart data"));
    }

    #[test]
    fn test_render_chart_area() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({
            "labels": ["Q1", "Q2", "Q3", "Q4"],
            "datasets": [{"data": [100, 150, 120, 180], "color": "#ed8936"}]
        });
        let block = CanvasBlock::chart(crate::document::ChartType::Area, data);
        let rendered = renderer.render_block(&block);

        assert!(rendered.html.contains("<polygon"));
        assert!(rendered.html.contains("<polyline"));
    }

    #[test]
    fn test_render_chart_scatter() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({
            "labels": ["P1", "P2", "P3"],
            "datasets": [{"data": [5, 10, 7], "color": "#805ad5"}]
        });
        let block = CanvasBlock::chart(crate::document::ChartType::Scatter, data);
        let rendered = renderer.render_block(&block);

        assert!(rendered.html.contains("<circle"));
        assert!(rendered.html.contains("opacity"));
    }

    #[test]
    fn test_render_chart_multiple_datasets() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({
            "labels": ["A", "B", "C"],
            "datasets": [
                {"data": [10, 20, 30], "color": "#4299e1"},
                {"data": [15, 25, 35], "color": "#48bb78"}
            ]
        });
        let block = CanvasBlock::chart(crate::document::ChartType::Line, data);
        let rendered = renderer.render_block(&block);

        // Should contain two polylines for two datasets
        let polyline_count = rendered.html.matches("<polyline").count();
        assert_eq!(polyline_count, 2);
    }

    #[test]
    fn test_render_chart_with_labels() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({
            "labels": ["January", "February", "March"],
            "datasets": [{"data": [10, 20, 30]}]
        });
        let block = CanvasBlock::chart(crate::document::ChartType::Bar, data);
        let rendered = renderer.render_block(&block);

        assert!(rendered.html.contains("January"));
        assert!(rendered.html.contains("February"));
        assert!(rendered.html.contains("March"));
    }

    #[test]
    fn test_render_chart_negative_values() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({
            "labels": ["A", "B", "C"],
            "datasets": [{"data": [-10, 20, -5]}]
        });
        let block = CanvasBlock::chart(crate::document::ChartType::Bar, data);
        let rendered = renderer.render_block(&block);

        // Should render without panic
        assert!(rendered.html.contains("<svg"));
    }

    #[test]
    fn test_chart_type_as_ref() {
        use crate::document::ChartType;

        assert_eq!(ChartType::Line.as_ref(), "line");
        assert_eq!(ChartType::Bar.as_ref(), "bar");
        assert_eq!(ChartType::Pie.as_ref(), "pie");
        assert_eq!(ChartType::Scatter.as_ref(), "scatter");
        assert_eq!(ChartType::Area.as_ref(), "area");
    }

    #[test]
    fn test_svg_dimensions() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({
            "labels": ["A"],
            "datasets": [{"data": [10]}]
        });
        let block = CanvasBlock::chart(crate::document::ChartType::Bar, data);
        let rendered = renderer.render_block(&block);

        assert!(rendered.html.contains("width=\"400\""));
        assert!(rendered.html.contains("height=\"250\""));
    }

    #[test]
    fn test_html_escape_special_chars_in_labels() {
        let renderer = ContentRenderer::new();
        let data = serde_json::json!({
            "labels": ["<script>", "A & B", "\"quoted\""],
            "datasets": [{"data": [10, 20, 30]}]
        });
        let block = CanvasBlock::chart(crate::document::ChartType::Bar, data);
        let rendered = renderer.render_block(&block);

        // HTML entities should be escaped
        assert!(rendered.html.contains("&lt;script&gt;"));
        assert!(rendered.html.contains("A &amp; B"));
    }
}
