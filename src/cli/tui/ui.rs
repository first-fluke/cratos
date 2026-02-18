//! TUI rendering with ratatui

use chrono::Timelike;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::app::{App, AppMode, Focus, Role};

const SPINNER_FRAMES: &[&str] = &["   ", ".  ", ".. ", "..."];

/// Main draw function — renders the full TUI layout.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Min(3),    // body
            Constraint::Length(1), // suggestions (optional)
            Constraint::Length(3), // input (height 3 to show border clearly)
        ])
        .split(frame.area());

    draw_status_bar(frame, app, outer[0]);
    draw_body(frame, app, outer[1]);
    
    // Only draw suggestions if we have them, otherwise reuse space? 
    // Layout constraints are fixed, so we just draw blank if empty.
    draw_suggestions(frame, app, outer[2]);
    draw_input(frame, app, outer[3]);
}

// ── status bar ──────────────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode_span = match app.ui_state.mode {
        AppMode::Normal => Span::styled(" NORMAL ", Style::default().bg(Color::Blue).fg(Color::Black).bold()),
        AppMode::Insert => Span::styled(" INSERT ", Style::default().bg(Color::Green).fg(Color::Black).bold()),
        AppMode::Command => Span::styled(" COMMAND ", Style::default().bg(Color::Yellow).fg(Color::Black).bold()),
    };

    let version = env!("CARGO_PKG_VERSION");
    let persona_display = capitalize(&app.persona);

    let mouse_mode = if app.ui_state.mouse_captured {
        "[F2: Mouse On]"
    } else {
        "[F2: Mouse Off]"
    };
    
    let info_text = format!(
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
    let right = right_parts.join(" \u{00b7} ");

    // Layout components
    let mut spans = vec![
        mode_span,
        Span::raw(info_text),
    ];

    // Align center/right is manual in standard Paragraph, but let's try just spacing
    // Getting area width to calculate padding
    let current_len: usize = spans.iter().map(|s| s.content.width()).sum();
    let center_len = center.width();
    let right_len = right.width();
    let width = area.width as usize;

    let total_used = current_len + center_len + right_len;
    let remaining = width.saturating_sub(total_used);
    
    // Distribute remaining space roughly equally
    let left_spacer = remaining / 2;
    let right_spacer = remaining.saturating_sub(left_spacer);

    if left_spacer > 0 { spans.push(Span::raw(" ".repeat(left_spacer))); }
    if !center.is_empty() {
        spans.push(Span::styled(center, Style::default().fg(Color::Yellow).bold()));
    }
    if right_spacer > 0 { spans.push(Span::raw(" ".repeat(right_spacer))); }
    if !right.is_empty() {
        spans.push(Span::styled(right, Style::default().fg(Color::Cyan)));
    }

    let line = Line::from(spans);
    let p = Paragraph::new(line).style(Style::default().bg(Color::Rgb(20, 20, 20)).fg(Color::White));
    frame.render_widget(p, area);
}

// ── body: chat + optional sidebar ──────────────────────────────────────

fn draw_body(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.ui_state.show_sidebar {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(20),    // chat area
                Constraint::Length(30), // sidebar (made wider)
            ])
            .split(area);
        draw_chat(frame, app, body[0]);
        draw_sidebar(frame, app, body[1]);
    } else {
        draw_chat(frame, app, area);
    }
}

fn draw_chat(frame: &mut Frame, app: &mut App, area: Rect) {
    let focus_style = if app.ui_state.focus == Focus::Chat {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(focus_style)
        .title(" Chat History ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut all_lines: Vec<Line> = Vec::new();
    for msg in &app.messages {
        let (role_style, role_name) = match msg.role {
            Role::User => (Style::default().bold(), "You".to_string()),
            Role::Assistant => (Style::default().fg(Color::Cyan).bold(), msg.sender.clone()),
            Role::System => (Style::default().fg(Color::Magenta).italic(), msg.sender.clone()),
        };

        let ts = msg.timestamp;
        let time_str = format!("{:02}:{:02}", ts.hour(), ts.minute());

        // Header: [12:30] Role
        all_lines.push(Line::from(vec![
            Span::styled(format!("[{}] ", time_str), Style::default().fg(Color::DarkGray)),
            Span::styled(role_name, role_style),
        ]));

        // Content
        if msg.role == Role::Assistant {
            let md_text = tui_markdown::from_str(&msg.content);
            for line in md_text.lines {
                let mut indented = vec![Span::raw("  ")]; // Indent
                indented.extend(line.spans);
                all_lines.push(Line::from(indented));
            }
        } else {
            let content_style = match msg.role {
                Role::System => Style::default().fg(Color::Gray),
                _ => Style::default(),
            };
            for line_text in msg.content.split('\n') {
                all_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(line_text.to_string(), content_style),
                ]));
            }
        }
        // Spacer between messages
        all_lines.push(Line::raw(""));
    }

    // Loading indicator inline
    if app.ui_state.is_loading {
        let dots = SPINNER_FRAMES[app.ui_state.loading_tick % SPINNER_FRAMES.len()];
        all_lines.push(Line::from(vec![
            Span::styled(format!("{} ", app.persona), Style::default().fg(Color::Magenta).bold()),
            Span::styled(format!("Thinking{}", dots), Style::default().fg(Color::DarkGray).italic()),
        ]));
    }

    let text = Text::from(all_lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

    // Scroll logic
    let total_lines = paragraph.line_count(inner.width) as u16;
    let view_height = inner.height;
    let max_scroll = total_lines.saturating_sub(view_height);
    
    // If scroll_offset is huge (from bottom/end key), clamp it
    let scroll_pos = if app.ui_state.scroll_offset > max_scroll as usize {
        0
    } else {
        max_scroll.saturating_sub(app.ui_state.scroll_offset as u16)
    };

    // Render scrollbar
    app.ui_state.scrollbar_state =
        ScrollbarState::new(max_scroll as usize).position((max_scroll - scroll_pos) as usize);

    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("\u{25b2}"))
            .end_symbol(Some("\u{25bc}"))
            .track_symbol(Some("\u{2502}"))
            .thumb_symbol("\u{2588}"),
        area,
        &mut app.ui_state.scrollbar_state,
    );

    frame.render_widget(paragraph.scroll((scroll_pos, 0)), inner);
}

fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let focus_style = if app.ui_state.focus == Focus::Sidebar {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(" Sidebar ")
        .borders(Borders::ALL)
        .border_style(focus_style);
    
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Reuse existing sidebar content logic roughly, but simpler layout
    // ...
    // Actually let's just render the quota info directly
    
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled("Persona:", Style::default().fg(Color::Cyan).bold())));
    lines.push(Line::from(format!("  {}", capitalize(&app.persona))));
    lines.push(Line::raw(""));

    lines.push(Line::from(Span::styled("Commands:", Style::default().fg(Color::Green).bold())));
    lines.push(Line::from("  /help   - Show help"));
    lines.push(Line::from("  /clear  - Clear chat"));
    lines.push(Line::from("  /quit   - Exit"));
    lines.push(Line::from("  Esc     - Normal Mode"));
    lines.push(Line::from("  i       - Insert Mode"));
    lines.push(Line::raw(""));

    lines.push(Line::from(Span::styled("Quotas:", Style::default().fg(Color::Yellow).bold())));
    if app.provider_quotas.is_empty() {
        lines.push(Line::from("  (awaiting data)"));
    } else {
        for pq in &app.provider_quotas {
            let color = quota_color(pq.remaining_pct);
            let tier = pq
                .tier_label
                .as_deref()
                .map(|t| format!(" [{}]", t))
                .unwrap_or_default();
            lines.push(Line::from(Span::styled(
                format!(" {}{}", capitalize(&pq.provider), tier),
                Style::default().fg(color).add_modifier(
                    if pq.remaining_pct.is_some_and(|p| p < 20.0) {
                        ratatui::style::Modifier::BOLD
                    } else {
                        ratatui::style::Modifier::empty()
                    },
                ),
            )));
            let detail = if pq.reset_display.is_empty() {
                format!("   {}", pq.summary)
            } else {
                format!("   {} {}", pq.summary, pq.reset_display)
            };
            lines.push(Line::from(Span::styled(detail, Style::default().fg(color))));
        }
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(p, inner);
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
                .bold(),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let focus_style = if app.ui_state.focus == Focus::Input {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = if app.ui_state.mode == AppMode::Insert {
        " Input (Insert) "
    } else {
        " Input (Normal) "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(focus_style)
        .title(title);

    let mut textarea = app.textarea.clone();
    textarea.set_block(block);
    
    if app.ui_state.mode != AppMode::Insert {
        textarea.set_cursor_style(Style::default().fg(Color::Reset)); 
    } 

    frame.render_widget(&textarea, area);
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
