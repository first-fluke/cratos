//! TUI rendering with ratatui

use chrono::Timelike;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::app::{App, Role};

const SPINNER_FRAMES: &[&str] = &["   ", ".  ", ".. ", "..."];

/// Main draw function — renders the full TUI layout.
pub fn draw(frame: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar (compact, no border)
            Constraint::Min(3),   // body (chat + optional sidebar)
            Constraint::Length(1), // input (compact, no border)
        ])
        .split(frame.area());

    draw_status_bar(frame, app, outer[0]);
    draw_body(frame, app, outer[1]);
    draw_input(frame, app, outer[2]);
}

// ── status bar ──────────────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let version = env!("CARGO_PKG_VERSION");
    let persona_display = capitalize(&app.persona);

    // Left: persona + provider + version
    let left = format!(
        " Cratos \u{00b7} {} \u{00b7} {} v{}",
        persona_display, app.provider_name, version,
    );

    // Center: loading indicator (if active)
    let center = if app.is_loading {
        let dots = SPINNER_FRAMES[app.loading_tick % SPINNER_FRAMES.len()];
        format!("Thinking{}", dots)
    } else {
        String::new()
    };

    // Right: quota
    let right = if app.quota_line.is_empty() {
        String::new()
    } else {
        format!("{} ", app.quota_line)
    };

    // Calculate spacing
    let left_w = left.width();
    let center_w = center.width();
    let right_w = right.width();
    let total_w = area.width as usize;

    // Build the status line with spacing
    let mut spans = vec![Span::styled(
        left,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )];

    // Gap between left and center
    let mid_point = total_w / 2;
    let center_start = mid_point.saturating_sub(center_w / 2);
    let gap1 = center_start.saturating_sub(left_w);
    if gap1 > 0 {
        spans.push(Span::raw(" ".repeat(gap1)));
    }

    if !center.is_empty() {
        spans.push(Span::styled(
            center,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Gap between center and right
    let used = left_w + gap1 + center_w;
    let gap2 = total_w.saturating_sub(used + right_w);
    if gap2 > 0 {
        spans.push(Span::raw(" ".repeat(gap2)));
    }

    if !right.is_empty() {
        spans.push(Span::styled(
            right,
            Style::default().fg(Color::DarkGray),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(
        Style::default().bg(Color::DarkGray).fg(Color::White),
    );
    frame.render_widget(paragraph, area);
}

// ── body: chat + optional sidebar ──────────────────────────────────────

fn draw_body(frame: &mut Frame, app: &App, area: Rect) {
    if app.show_sidebar {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(20),    // chat area (flexible)
                Constraint::Length(22), // sidebar (fixed)
            ])
            .split(area);
        draw_chat(frame, app, body[0]);
        draw_sidebar(frame, app, body[1]);
    } else {
        draw_chat(frame, app, area);
    }
}

fn draw_chat(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Chat ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Build all lines from messages
    let mut all_lines: Vec<Line> = Vec::new();
    for msg in &app.messages {
        let (prefix_style, prefix) = match msg.role {
            Role::User => (
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
                "You: ".to_string(),
            ),
            Role::Assistant => (
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                format!("{}: ", msg.sender),
            ),
            Role::System => (
                Style::default().fg(Color::Yellow),
                format!("[{}] ", msg.sender),
            ),
        };

        let content_style = match msg.role {
            Role::User => Style::default().fg(Color::White),
            Role::Assistant => Style::default().fg(Color::Cyan),
            Role::System => Style::default().fg(Color::Yellow),
        };

        let ts = msg.timestamp;
        let time_str = format!("{:02}:{:02} ", ts.hour(), ts.minute());

        // Split content by newlines so multi-line messages render properly
        let indent_len = time_str.len() + prefix.width();
        for (i, line_text) in msg.content.split('\n').enumerate() {
            if i == 0 {
                all_lines.push(Line::from(vec![
                    Span::styled(time_str.clone(), Style::default().fg(Color::DarkGray)),
                    Span::styled(prefix.clone(), prefix_style),
                    Span::styled(line_text.to_string(), content_style),
                ]));
            } else {
                // Continuation lines: indent to align with content
                all_lines.push(Line::from(vec![
                    Span::raw(" ".repeat(indent_len)),
                    Span::styled(line_text.to_string(), content_style),
                ]));
            }
        }
    }

    // Loading indicator as inline chat line
    if app.is_loading {
        let dots = SPINNER_FRAMES[app.loading_tick % SPINNER_FRAMES.len()];
        all_lines.push(Line::from(vec![
            Span::styled(
                format!("{}: ", app.persona),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("Thinking{}", dots),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::DIM),
            ),
        ]));
    }

    let text = Text::from(all_lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

    // Use Paragraph::line_count to compute wrapped line total for scroll
    let total_wrapped = paragraph.line_count(inner.width) as u16;
    let max_scroll = total_wrapped.saturating_sub(inner.height);
    let scroll_pos = max_scroll.saturating_sub(app.scroll_offset as u16);

    let paragraph = paragraph.scroll((scroll_pos, 0));
    frame.render_widget(paragraph, inner);
}

fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let has_quota = !app.quota_line.is_empty();
    let quota_height = if has_quota { 5 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),            // persona
            Constraint::Min(4),              // commands
            Constraint::Length(quota_height), // quota (0 if no data)
        ])
        .split(area);

    // Persona block
    let persona_display = capitalize(&app.persona);
    let persona_text = Text::from(vec![
        Line::from(Span::styled(
            format!(" {}", persona_display),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            " Active persona",
            Style::default().fg(Color::DarkGray),
        )),
    ]);

    let persona_block = Paragraph::new(persona_text).block(
        Block::default()
            .title(" Persona ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(persona_block, chunks[0]);

    // Commands block
    let cmds = vec![
        Line::from(Span::styled(
            " /persona <n>",
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            " /clear",
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            " /help",
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            " /quit",
            Style::default().fg(Color::Green),
        )),
    ];

    let cmd_block = Paragraph::new(cmds).block(
        Block::default()
            .title(" Commands ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(cmd_block, chunks[1]);

    // Quota block (only if data available)
    if has_quota {
        let quota_text = Text::from(vec![
            Line::from(Span::styled(
                format!(" {}", app.quota_line),
                Style::default().fg(Color::DarkGray),
            )),
        ]);
        let quota_block = Paragraph::new(quota_text).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(" Quota ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(quota_block, chunks[2]);
    }
}

// ── input bar ───────────────────────────────────────────────────────────

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let prompt = Span::styled(
        "cratos> ",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    let input_text = Span::raw(&app.input);
    let line = Line::from(vec![prompt, input_text]);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);

    // CJK-aware cursor position
    let prompt_len = "cratos> ".len() as u16;
    let display_pos = app.input[..app.cursor_pos].width() as u16;
    frame.set_cursor_position((area.x + prompt_len + display_pos, area.y));
}

// ── utils ───────────────────────────────────────────────────────────────

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
