//! Chart Component
//!
//! Renders charts using SVG. Pure Rust implementation without external JS libraries.
//! Uses a lightweight SVG-based approach for WASM compatibility.

use leptos::*;
use serde::{Deserialize, Serialize};

// Chart constants
const DEFAULT_WIDTH: u32 = 640;
const DEFAULT_HEIGHT: u32 = 400;
const PADDING: u32 = 60;
const CHART_COLORS: [&str; 8] = [
    "#3B82F6", // blue
    "#8B5CF6", // purple
    "#10B981", // green
    "#F59E0B", // yellow
    "#EF4444", // red
    "#EC4899", // pink
    "#06B6D4", // cyan
    "#F97316", // orange
];

// SVG color constants - updated for glassmorphism
const BG_COLOR: &str = "transparent";
const GRID_COLOR: &str = "rgba(255, 255, 255, 0.05)";
const AXIS_COLOR: &str = "rgba(255, 255, 255, 0.1)";
const TEXT_COLOR: &str = "rgba(255, 255, 255, 0.5)";

/// Chart data structure
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChartData {
    /// Chart title
    pub title: String,
    /// Data labels (x-axis)
    pub labels: Vec<String>,
    /// Data series
    pub series: Vec<DataSeries>,
}

/// Single data series
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataSeries {
    /// Series name
    pub name: String,
    /// Data values
    pub values: Vec<f64>,
    /// Optional custom color
    pub color: Option<String>,
}

/// Chart type
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ChartType {
    #[default]
    Line,
    Bar,
    Area,
    Scatter,
    Pie,
}

impl ChartType {
    /// Parse from string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "line" => ChartType::Line,
            "bar" => ChartType::Bar,
            "area" => ChartType::Area,
            "scatter" => ChartType::Scatter,
            "pie" => ChartType::Pie,
            _ => ChartType::Line,
        }
    }
}

/// Chart component
#[component]
pub fn Chart(
    /// Chart data
    data: ChartData,
    /// Chart type
    #[prop(optional, default = ChartType::Line)]
    chart_type: ChartType,
    /// Chart width
    #[prop(optional, default = DEFAULT_WIDTH)]
    width: u32,
    /// Chart height
    #[prop(optional, default = DEFAULT_HEIGHT)]
    height: u32,
    /// Show legend
    #[prop(optional, default = true)]
    show_legend: bool,
    /// Show grid lines
    #[prop(optional, default = true)]
    show_grid: bool,
) -> impl IntoView {
    let svg_content = match chart_type {
        ChartType::Line => render_line_chart(&data, width, height, show_grid),
        ChartType::Bar => render_bar_chart(&data, width, height, show_grid),
        ChartType::Area => render_area_chart(&data, width, height, show_grid),
        ChartType::Scatter => render_scatter_chart(&data, width, height, show_grid),
        ChartType::Pie => render_pie_chart(&data, width, height),
    };

    // Clone data for use in view
    let title = data.title.clone();
    let title_for_check = title.clone();
    let series = data.series.clone();
    let series_for_check = series.clone();

    view! {
        <div class="chart-container rounded-2xl overflow-hidden transition-all duration-500">
            // Title
            <Show when=move || !title_for_check.is_empty()>
                <div class="px-4 py-3 border-b border-white/5">
                    <h3 class="text-sm font-bold tracking-wider uppercase text-theme-secondary opacity-70">{title.clone()}</h3>
                </div>
            </Show>

            // Chart SVG
            <div class="p-2 md:p-4" inner_html=svg_content.clone() />

            // Legend
            <Show when=move || show_legend && !series_for_check.is_empty()>
                <div class="px-6 pb-6 flex flex-wrap gap-6 mt-2">
                    {series.iter().enumerate().map(|(i, series_item)| {
                        let color = series_item.color.clone().unwrap_or_else(|| CHART_COLORS[i % CHART_COLORS.len()].to_string());
                        let name = series_item.name.clone();
                        view! {
                            <div class="flex items-center space-x-3 group cursor-default">
                                <div
                                    class="w-2.5 h-2.5 rounded-full shadow-[0_0_8px_rgba(0,0,0,0.2)] group-hover:scale-125 transition-transform"
                                    style=format!("background-color: {}; box-shadow: 0 0 10px {}44", color, color)
                                />
                                <span class="text-[10px] font-black uppercase tracking-widest text-theme-secondary group-hover:text-theme-primary transition-colors">{name}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </Show>
        </div>
    }
}

/// Simple line chart component for quick usage
#[component]
pub fn SimpleLineChart(
    /// Chart title
    #[prop(into)]
    title: String,
    /// Data labels
    labels: Vec<String>,
    /// Data values
    values: Vec<f64>,
    /// Optional color
    #[prop(optional, into)]
    color: Option<String>,
) -> impl IntoView {
    let data = ChartData {
        title,
        labels,
        series: vec![DataSeries {
            name: "Data".to_string(),
            values,
            color,
        }],
    };

    view! {
        <Chart data=data chart_type=ChartType::Line />
    }
}

/// Simple bar chart component for quick usage
#[component]
pub fn SimpleBarChart(
    /// Chart title
    #[prop(into)]
    title: String,
    /// Data labels
    labels: Vec<String>,
    /// Data values
    values: Vec<f64>,
    /// Optional color
    #[prop(optional, into)]
    color: Option<String>,
) -> impl IntoView {
    let data = ChartData {
        title,
        labels,
        series: vec![DataSeries {
            name: "Data".to_string(),
            values,
            color,
        }],
    };

    view! {
        <Chart data=data chart_type=ChartType::Bar />
    }
}

// SVG Rendering functions

fn render_line_chart(data: &ChartData, width: u32, height: u32, show_grid: bool) -> String {
    let chart_width = width - 2 * PADDING;
    let chart_height = height - 2 * PADDING;

    let (min_val, max_val) = get_value_range(data);
    let value_range = max_val - min_val;

    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" class="w-full h-auto">"##,
        width, height
    );

    // Background
    svg.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="{}"/>"##,
        width, height, BG_COLOR
    ));

    // Grid lines
    if show_grid {
        svg.push_str(&render_grid(width, height, PADDING, 5, 5));
    }

    // Axes
    svg.push_str(&render_axes(width, height, PADDING));

    // Y-axis labels
    svg.push_str(&render_y_labels(height, PADDING, min_val, max_val, 5));

    // X-axis labels
    svg.push_str(&render_x_labels(width, height, PADDING, &data.labels));

    // Data lines
    for (series_idx, series) in data.series.iter().enumerate() {
        let color = series.color.as_deref()
            .unwrap_or(CHART_COLORS[series_idx % CHART_COLORS.len()]);

        let points: Vec<String> = series.values.iter().enumerate().map(|(i, &val)| {
            let x = PADDING + (i as f64 * chart_width as f64 / (series.values.len() - 1).max(1) as f64) as u32;
            let y = height - PADDING - ((val - min_val) / value_range * chart_height as f64) as u32;
            format!("{},{}", x, y)
        }).collect();

        if !points.is_empty() {
            svg.push_str(&format!(
                r##"<polyline fill="none" stroke="{}" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" points="{}"/>"##,
                color,
                points.join(" ")
            ));

            // Data points
            for point in &points {
                let coords: Vec<&str> = point.split(',').collect();
                if coords.len() == 2 {
                    svg.push_str(&format!(
                        r##"<circle cx="{}" cy="{}" r="5" fill="{}" stroke="{}" stroke-width="2"/>"##,
                        coords[0], coords[1], color, "white"
                    ));
                }
            }
        }
    }

    svg.push_str("</svg>");
    svg
}

fn render_bar_chart(data: &ChartData, width: u32, height: u32, show_grid: bool) -> String {
    let chart_width = width - 2 * PADDING;
    let chart_height = height - 2 * PADDING;

    let (min_val, max_val) = get_value_range(data);
    let value_range = max_val - min_val.min(0.0);
    let baseline = min_val.min(0.0);

    let num_bars = data.labels.len();
    let num_series = data.series.len();
    let bar_group_width = chart_width as f64 / num_bars as f64;
    let bar_width = (bar_group_width * 0.7) / num_series as f64;
    let bar_gap = bar_group_width * 0.15;

    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" class="w-full h-auto">"##,
        width, height
    );

    // Background
    svg.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="{}"/>"##,
        width, height, BG_COLOR
    ));

    // Grid lines
    if show_grid {
        svg.push_str(&render_grid(width, height, PADDING, 5, 0));
    }

    // Axes
    svg.push_str(&render_axes(width, height, PADDING));

    // Y-axis labels
    svg.push_str(&render_y_labels(height, PADDING, baseline, max_val, 5));

    // X-axis labels
    svg.push_str(&render_x_labels(width, height, PADDING, &data.labels));

    // Bars
    for (i, label) in data.labels.iter().enumerate() {
        let _ = label; // Used for accessibility
        let group_x = PADDING as f64 + i as f64 * bar_group_width + bar_gap;

        for (series_idx, series) in data.series.iter().enumerate() {
            if let Some(&val) = series.values.get(i) {
                let color = series.color.as_deref()
                    .unwrap_or(CHART_COLORS[series_idx % CHART_COLORS.len()]);

                let x = group_x + series_idx as f64 * bar_width;
                let bar_height = ((val - baseline) / value_range * chart_height as f64).abs();
                let y = if val >= 0.0 {
                    height as f64 - PADDING as f64 - bar_height
                } else {
                    height as f64 - PADDING as f64
                };

                svg.push_str(&format!(
                    r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{}" rx="4" fill-opacity="0.8"/>"##,
                    x, y, bar_width, bar_height, color
                ));
            }
        }
    }

    svg.push_str("</svg>");
    svg
}

fn render_area_chart(data: &ChartData, width: u32, height: u32, show_grid: bool) -> String {
    let chart_width = width - 2 * PADDING;
    let chart_height = height - 2 * PADDING;

    let (min_val, max_val) = get_value_range(data);
    let value_range = max_val - min_val;

    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" class="w-full h-auto">"##,
        width, height
    );

    // Background
    svg.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="{}"/>"##,
        width, height, BG_COLOR
    ));

    // Grid lines
    if show_grid {
        svg.push_str(&render_grid(width, height, PADDING, 5, 5));
    }

    // Axes
    svg.push_str(&render_axes(width, height, PADDING));

    // Y-axis labels
    svg.push_str(&render_y_labels(height, PADDING, min_val, max_val, 5));

    // X-axis labels
    svg.push_str(&render_x_labels(width, height, PADDING, &data.labels));

    // Data areas (drawn in reverse order for proper layering)
    for (series_idx, series) in data.series.iter().enumerate().rev() {
        let color = series.color.as_deref()
            .unwrap_or(CHART_COLORS[series_idx % CHART_COLORS.len()]);

        let mut path = String::new();
        let baseline_y = height - PADDING;

        // Start from baseline
        path.push_str(&format!("M{},{}", PADDING, baseline_y));

        // Draw line to each point
        for (i, &val) in series.values.iter().enumerate() {
            let x = PADDING + (i as f64 * chart_width as f64 / (series.values.len() - 1).max(1) as f64) as u32;
            let y = height - PADDING - ((val - min_val) / value_range * chart_height as f64) as u32;
            path.push_str(&format!("L{},{}", x, y));
        }

        // Close path back to baseline
        if !series.values.is_empty() {
            let last_x = PADDING + chart_width;
            path.push_str(&format!("L{},{}", last_x, baseline_y));
        }
        path.push('Z');

        svg.push_str(&format!(
            r##"
            <defs>
                <linearGradient id="grad-{}" x1="0%" y1="0%" x2="0%" y2="100%">
                    <stop offset="0%" style="stop-color:{};stop-opacity:0.4" />
                    <stop offset="100%" style="stop-color:{};stop-opacity:0.05" />
                </linearGradient>
            </defs>
            <path d="{}" fill="url(#grad-{})" stroke="{}" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>"##,
            series_idx, color, color, path, series_idx, color
        ));
    }

    svg.push_str("</svg>");
    svg
}

fn render_scatter_chart(data: &ChartData, width: u32, height: u32, show_grid: bool) -> String {
    let chart_width = width - 2 * PADDING;
    let chart_height = height - 2 * PADDING;

    let (min_val, max_val) = get_value_range(data);
    let value_range = max_val - min_val;

    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" class="w-full h-auto">"##,
        width, height
    );

    // Background
    svg.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="{}"/>"##,
        width, height, BG_COLOR
    ));

    // Grid lines
    if show_grid {
        svg.push_str(&render_grid(width, height, PADDING, 5, 5));
    }

    // Axes
    svg.push_str(&render_axes(width, height, PADDING));

    // Y-axis labels
    svg.push_str(&render_y_labels(height, PADDING, min_val, max_val, 5));

    // X-axis labels
    svg.push_str(&render_x_labels(width, height, PADDING, &data.labels));

    // Data points
    for (series_idx, series) in data.series.iter().enumerate() {
        let color = series.color.as_deref()
            .unwrap_or(CHART_COLORS[series_idx % CHART_COLORS.len()]);

        for (i, &val) in series.values.iter().enumerate() {
            let x = PADDING + (i as f64 * chart_width as f64 / (series.values.len() - 1).max(1) as f64) as u32;
            let y = height - PADDING - ((val - min_val) / value_range * chart_height as f64) as u32;

            svg.push_str(&format!(
                r##"<circle cx="{}" cy="{}" r="6" fill="{}" stroke="rgba(255,255,255,0.8)" stroke-width="2" fill-opacity="0.7"/>"##,
                x, y, color
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

fn render_pie_chart(data: &ChartData, width: u32, height: u32) -> String {
    let cx = width as f64 / 2.0;
    let cy = height as f64 / 2.0;
    let radius = (width.min(height) as f64 - 2.0 * PADDING as f64) / 2.0;

    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" class="w-full h-auto">"##,
        width, height
    );

    // Background
    svg.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="{}"/>"##,
        width, height, BG_COLOR
    ));

    // Calculate total
    let total: f64 = data.series.first()
        .map(|s| s.values.iter().sum())
        .unwrap_or(0.0);

    if total <= 0.0 || data.labels.is_empty() {
        svg.push_str("</svg>");
        return svg;
    }

    let empty_vec: Vec<f64> = Vec::new();
    let values = data.series.first()
        .map(|s| &s.values)
        .unwrap_or(&empty_vec);

    let mut start_angle = -std::f64::consts::FRAC_PI_2; // Start at top

    for (i, (label, &value)) in data.labels.iter().zip(values.iter()).enumerate() {
        let _ = label;
        let angle = (value / total) * 2.0 * std::f64::consts::PI;
        let end_angle = start_angle + angle;

        let color = CHART_COLORS[i % CHART_COLORS.len()];

        // Calculate arc path
        let x1 = cx + radius * start_angle.cos();
        let y1 = cy + radius * start_angle.sin();
        let x2 = cx + radius * end_angle.cos();
        let y2 = cy + radius * end_angle.sin();

        let large_arc = if angle > std::f64::consts::PI { 1 } else { 0 };

        svg.push_str(&format!(
            r##"<path d="M{},{} L{:.1},{:.1} A{},{} 0 {} 1 {:.1},{:.1} Z" fill="{}" fill-opacity="0.8" stroke="white" stroke-opacity="0.2" stroke-width="2"/>"##,
            cx, cy, x1, y1, radius, radius, large_arc, x2, y2, color
        ));

        start_angle = end_angle;
    }

    svg.push_str("</svg>");
    svg
}

// Helper functions

fn get_value_range(data: &ChartData) -> (f64, f64) {
    let mut min_val = f64::MAX;
    let mut max_val = f64::MIN;

    for series in &data.series {
        for &val in &series.values {
            min_val = min_val.min(val);
            max_val = max_val.max(val);
        }
    }

    if min_val == f64::MAX {
        min_val = 0.0;
    }
    if max_val == f64::MIN {
        max_val = 100.0;
    }

    // Add padding to range
    let padding = (max_val - min_val) * 0.15;
    (min_val - padding, max_val + padding)
}

fn render_grid(width: u32, height: u32, padding: u32, y_lines: u32, x_lines: u32) -> String {
    let chart_width = width - 2 * padding;
    let chart_height = height - 2 * padding;
    let mut svg = String::new();

    // Horizontal lines
    for i in 0..=y_lines {
        let y = padding + (i as f64 * chart_height as f64 / y_lines as f64) as u32;
        svg.push_str(&format!(
            r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-dasharray="2,4"/>"##,
            padding, y, padding + chart_width, y, GRID_COLOR
        ));
    }

    // Vertical lines
    if x_lines > 0 {
        for i in 0..=x_lines {
            let x = padding + (i as f64 * chart_width as f64 / x_lines as f64) as u32;
            svg.push_str(&format!(
                r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-dasharray="2,4"/>"##,
                x, padding, x, height - padding, GRID_COLOR
            ));
        }
    }

    svg
}

fn render_axes(width: u32, height: u32, padding: u32) -> String {
    format!(
        r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-opacity="0.3"/>
           <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-opacity="0.3"/>"##,
        padding, padding, padding, height - padding, AXIS_COLOR, // Y-axis
        padding, height - padding, width - padding, height - padding, AXIS_COLOR // X-axis
    )
}

fn render_y_labels(height: u32, padding: u32, min_val: f64, max_val: f64, num_labels: u32) -> String {
    let chart_height = height - 2 * padding;
    let mut svg = String::new();

    for i in 0..=num_labels {
        let y = height - padding - (i as f64 * chart_height as f64 / num_labels as f64) as u32;
        let val = min_val + (i as f64 * (max_val - min_val) / num_labels as f64);

        svg.push_str(&format!(
            r##"<text x="{}" y="{}" text-anchor="end" fill="{}" font-size="10" font-weight="bold" font-family="monospace">{:.1}</text>"##,
            padding - 12, y + 4, TEXT_COLOR, val
        ));
    }

    svg
}

fn render_x_labels(width: u32, height: u32, padding: u32, labels: &[String]) -> String {
    if labels.is_empty() {
        return String::new();
    }

    let chart_width = width - 2 * padding;
    let mut svg = String::new();

    for (i, label) in labels.iter().enumerate() {
        let x = padding + (i as f64 * chart_width as f64 / (labels.len() - 1).max(1) as f64) as u32;
        let y = height - padding + 24;

        // Truncate long labels
        let display_label = if label.len() > 10 {
            format!("{}...", &label[..7])
        } else {
            label.clone()
        };

        svg.push_str(&format!(
            r##"<text x="{}" y="{}" text-anchor="middle" fill="{}" font-size="10" font-weight="bold" font-family="monospace">{}</text>"##,
            x, y, TEXT_COLOR, display_label
        ));
    }

    svg
}