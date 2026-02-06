//! TUI rendering with ratatui

use chrono::Timelike;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{App, Role};

const SPINNER_FRAMES: &[&str] = &["   ", ".  ", ".. ", "..."];

/// Main draw function — renders the full TUI layout.
pub fn draw(frame: &mut Frame, app: &App) {
    // Three vertical sections: status bar, main body, input bar.
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // status bar
            Constraint::Min(5),    // body
            Constraint::Length(3), // input
        ])
        .split(frame.area());

    draw_status_bar(frame, app, outer[0]);
    draw_body(frame, app, outer[1]);
    draw_input(frame, app, outer[2]);
}

// ── status bar ──────────────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let version = env!("CARGO_PKG_VERSION");
    let status_text = format!(
        " Cratos Chat        Provider: {}  |  v{}",
        app.provider_name, version,
    );

    let paragraph = Paragraph::new(status_text).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(paragraph, area);
}

// ── body: chat + sidebar ────────────────────────────────────────────────

fn draw_body(frame: &mut Frame, app: &App, area: Rect) {
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),      // chat area (flexible)
            Constraint::Length(22),    // sidebar (fixed)
        ])
        .split(area);

    draw_chat(frame, app, body[0]);
    draw_sidebar(frame, app, body[1]);
}

fn draw_chat(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Chat ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build list items from messages.
    let mut items: Vec<ListItem> = app
        .messages
        .iter()
        .map(|msg| {
            let (prefix_style, prefix) = match msg.role {
                Role::User => (
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    "You: ".to_string(),
                ),
                Role::Assistant => (
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
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

            let line = Line::from(vec![
                Span::styled(time_str, Style::default().fg(Color::DarkGray)),
                Span::styled(prefix, prefix_style),
                Span::styled(&msg.content, content_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    // Append a loading indicator when waiting for LLM.
    if app.is_loading {
        let dots = SPINNER_FRAMES[app.loading_tick % SPINNER_FRAMES.len()];
        let line = Line::from(vec![
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
        ]);
        items.push(ListItem::new(line));
    }

    // Calculate visible window with scrolling from bottom.
    let visible_height = inner.height as usize;
    let total = items.len();
    let end = total.saturating_sub(app.scroll_offset);
    let start = end.saturating_sub(visible_height);

    let visible_items: Vec<ListItem> = items.into_iter().skip(start).take(end - start).collect();
    let list = List::new(visible_items);
    frame.render_widget(list, inner);
}

fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // persona
            Constraint::Min(4),   // commands
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
        Line::from(Span::styled(" /persona <n>", Style::default().fg(Color::Green))),
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
}

// ── input bar ───────────────────────────────────────────────────────────

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let prompt = Span::styled(
        " cratos> ",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    let input_text = Span::raw(&app.input);
    let line = Line::from(vec![prompt, input_text]);

    let paragraph = Paragraph::new(line).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);

    // Place the cursor.
    let prompt_len = " cratos> ".len() as u16;
    let cursor_x = inner.x + prompt_len + app.cursor_pos as u16;
    let cursor_y = inner.y;
    frame.set_cursor_position((cursor_x, cursor_y));
}

// ── utils ───────────────────────────────────────────────────────────────

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
