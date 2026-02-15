//! TUI rendering with ratatui

use chrono::Timelike;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::app::{App, Role};

const SPINNER_FRAMES: &[&str] = &["   ", ".  ", ".. ", "..."];

/// Main draw function — renders the full TUI layout.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Min(3),    // body
            Constraint::Length(1), // suggestions
            Constraint::Length(1), // input
        ])
        .split(frame.area());

    draw_status_bar(frame, app, outer[0]);
    draw_body(frame, app, outer[1]);
    draw_suggestions(frame, app, outer[2]);
    draw_input(frame, app, outer[3]);
}

// ── status bar ──────────────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let version = env!("CARGO_PKG_VERSION");
    let persona_display = capitalize(&app.persona);

    let mouse_mode = if app.ui_state.mouse_captured {
        "[Scroll]"
    } else {
        "[Select]"
    };
    let left = format!(
        " Cratos \u{00b7} {} \u{00b7} {} v{} {}",
        persona_display, app.provider_name, version, mouse_mode,
    );

    let center = if app.ui_state.is_loading {
        let dots = SPINNER_FRAMES[app.ui_state.loading_tick % SPINNER_FRAMES.len()];
        format!("Thinking{}", dots)
    } else {
        String::new()
    };

    let mut right_parts: Vec<String> = Vec::new();
    if !app.cost_line.is_empty() {
        right_parts.push(app.cost_line.clone());
    }
    if !app.quota_status_line.is_empty() {
        right_parts.push(app.quota_status_line.clone());
    }
    let right = if right_parts.is_empty() {
        String::new()
    } else {
        format!("{} ", right_parts.join(" \u{00b7} "))
    };

    let left_w = left.width();
    let center_w = center.width();
    let right_w = right.width();
    let total_w = area.width as usize;

    let mut spans = vec![Span::styled(
        left,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )];

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

    let used = left_w + gap1 + center_w;
    let gap2 = total_w.saturating_sub(used + right_w);
    if gap2 > 0 {
        spans.push(Span::raw(" ".repeat(gap2)));
    }

    if !right.is_empty() {
        spans.push(Span::styled(right, Style::default().fg(Color::LightCyan)));
    }

    let line = Line::from(spans);
    let paragraph =
        Paragraph::new(line).style(Style::default().bg(Color::Rgb(40, 44, 52)).fg(Color::White));
    frame.render_widget(paragraph, area);
}

// ── body: chat + optional sidebar ──────────────────────────────────────

fn draw_body(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.ui_state.show_sidebar {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(20),    // chat area
                Constraint::Length(24), // sidebar
            ])
            .split(area);
        draw_chat(frame, app, body[0]);
        draw_sidebar(frame, app, body[1]);
    } else {
        draw_chat(frame, app, area);
    }
}

fn draw_chat(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .title(" Chat History ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut all_lines: Vec<Line> = Vec::new();
    for msg in &app.messages {
        let (prefix_style, prefix) = match msg.role {
            Role::User => (
                Style::default()
                    .fg(Color::Green)
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
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
                format!("[{}] ", msg.sender),
            ),
        };

        let ts = msg.timestamp;
        let time_str = format!("{:02}:{:02} ", ts.hour(), ts.minute());
        let indent_len = time_str.len() + prefix.width();

        all_lines.push(Line::from(vec![
            Span::styled(time_str.clone(), Style::default().fg(Color::DarkGray)),
            Span::styled(prefix.clone(), prefix_style),
        ]));

        if msg.role == Role::Assistant {
            let md_text = tui_markdown::from_str(&msg.content);
            for line in md_text.lines {
                let mut indented = vec![Span::raw(" ".repeat(indent_len))];
                indented.extend(line.spans);
                all_lines.push(Line::from(indented));
            }
        } else {
            let content_style = match msg.role {
                Role::User => Style::default().fg(Color::White),
                Role::System => Style::default().fg(Color::DarkGray),
                _ => Style::default(),
            };
            for line_text in msg.content.split('\n') {
                all_lines.push(Line::from(vec![
                    Span::raw(" ".repeat(indent_len)),
                    Span::styled(line_text.to_string(), content_style),
                ]));
            }
        }
    }

    if app.ui_state.is_loading {
        let dots = SPINNER_FRAMES[app.ui_state.loading_tick % SPINNER_FRAMES.len()];
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

    let total_wrapped = paragraph.line_count(inner.width) as u16;
    let max_scroll = total_wrapped.saturating_sub(inner.height);
    let scroll_pos = max_scroll.saturating_sub(app.ui_state.scroll_offset as u16);

    // Render scrollbar
    app.ui_state.scrollbar_state =
        ScrollbarState::new(max_scroll as usize).position((max_scroll - scroll_pos) as usize);

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("\u{25b2}"))
        .end_symbol(Some("\u{25bc}"))
        .track_symbol(Some("\u{2502}"))
        .thumb_symbol("\u{2588}");

    frame.render_stateful_widget(scrollbar, area, &mut app.ui_state.scrollbar_state);

    let paragraph = paragraph.scroll((scroll_pos, 0));
    frame.render_widget(paragraph, inner);
}

fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let quota_lines = if app.provider_quotas.is_empty() {
        1
    } else {
        app.provider_quotas.len() * 2
    };
    let quota_height = (quota_lines + 2) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),            // persona
            Constraint::Min(4),               // commands
            Constraint::Length(quota_height), // quota
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
        Line::from(Span::styled(" /clear", Style::default().fg(Color::Green))),
        Line::from(Span::styled(" /help", Style::default().fg(Color::Green))),
        Line::from(Span::styled(" /quit", Style::default().fg(Color::Green))),
    ];

    let cmd_block = Paragraph::new(cmds).block(
        Block::default()
            .title(" Commands ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(cmd_block, chunks[1]);

    // Quota block
    let mut quota_lines_vec: Vec<Line> = Vec::new();
    if app.provider_quotas.is_empty() {
        quota_lines_vec.push(Line::from(Span::styled(
            " awaiting data",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for pq in &app.provider_quotas {
            let color = quota_color(pq.remaining_pct);
            let tier = pq
                .tier_label
                .as_deref()
                .map(|t| format!(" [{}]", t))
                .unwrap_or_default();
            quota_lines_vec.push(Line::from(Span::styled(
                format!(" {}{}", capitalize(&pq.provider), tier),
                Style::default().fg(color).add_modifier(
                    if pq.remaining_pct.is_some_and(|p| p < 20.0) {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    },
                ),
            )));
            let detail = if pq.reset_display.is_empty() {
                format!("   {}", pq.summary)
            } else {
                format!("   {} {}", pq.summary, pq.reset_display)
            };
            quota_lines_vec.push(Line::from(Span::styled(detail, Style::default().fg(color))));
        }
    }

    let quota_block = Paragraph::new(quota_lines_vec)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(" Quota ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(quota_block, chunks[2]);
}

fn draw_suggestions(frame: &mut Frame, app: &App, area: Rect) {
    if app.ui_state.suggestions.is_empty() {
        return;
    }

    let mut spans = vec![Span::styled(
        "Suggest: ",
        Style::default().fg(Color::DarkGray),
    )];
    for suggest in &app.ui_state.suggestions {
        spans.push(Span::styled(
            format!("/{} ", suggest),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(8), // "cratos> "
            Constraint::Min(1),    // textarea
        ])
        .split(area);

    let prompt_color = if app.ui_state.is_loading {
        Color::DarkGray
    } else {
        Color::Yellow
    };

    let prompt = Paragraph::new(Span::styled(
        "cratos> ",
        Style::default()
            .fg(prompt_color)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(prompt, chunks[0]);
    frame.render_widget(&app.textarea, chunks[1]);
}

fn quota_color(remaining_pct: Option<f64>) -> Color {
    match remaining_pct {
        Some(p) if p < 20.0 => Color::Red,
        Some(p) if p < 50.0 => Color::Yellow,
        Some(_) => Color::Green,
        None => Color::DarkGray,
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
