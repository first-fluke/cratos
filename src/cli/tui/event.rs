//! Crossterm event handling for the TUI

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use crossterm::execute;
use std::time::Duration;

use super::app::{App, AppMode, Focus};

/// Mouse scroll lines per event.
const MOUSE_SCROLL_LINES: u32 = 3;

/// Poll crossterm events and update app state.
/// Returns `true` if the app should quit.
pub fn handle_events(app: &mut App, timeout: Duration) -> Result<()> {
    // Drain any pending LLM responses first.
    app.poll_responses();

    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) => handle_key(app, key),
            Event::Mouse(mouse) => handle_mouse(app, mouse),
            _ => {}
        }
    }

    // Advance the spinner.
    app.tick();

    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) {
    match app.ui_state.mode {
        AppMode::Insert => handle_insert_mode(app, key),
        AppMode::Normal => handle_normal_mode(app, key),
        AppMode::Command => handle_command_mode(app, key),
    }
}

fn handle_insert_mode(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        // ── Global Quit being available in Insert is debated, but let's keep Ctrl+C
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            if app.has_active_execution() {
                app.abort_current_execution();
            } else {
                app.should_quit = true;
            }
        }

        // ── Switch to Normal Mode ──
        (_, KeyCode::Esc) => {
            app.set_mode(AppMode::Normal);
        }

        // ── Submit ──
        (_, KeyCode::Enter) => {
            // Prevent submitting if shifting (for newline) - optional, but standard Enter submits
            if !app.ui_state.is_loading {
                app.submit();
            }
        }

        // ── History Navigation (Optional in Insert, but useful) ──
        (_, KeyCode::Up) if app.is_input_empty() => {
            // If empty, standard Up behavior (history?)
            if app.has_history() {
                app.history_up();
            }
        }
        (_, KeyCode::Down) if app.is_input_empty() => {
            app.history_down();
        }

        // ── Tab Completion ──
        (_, KeyCode::Tab) => {
            if !app.ui_state.suggestions.is_empty() {
                let suggestion = app.ui_state.suggestions[0];
                app.textarea = tui_textarea::TextArea::new(vec![format!("/{}", suggestion)]);
                app.textarea.move_cursor(tui_textarea::CursorMove::End);
                app.ui_state.suggestions.clear();
            }
        }

        // ── Text Input ──
        _ => {
            app.textarea.input(Event::Key(key));
            app.update_suggestions();
        }
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        // ── Quit ──
        (KeyModifiers::CONTROL, KeyCode::Char('c')) | (_, KeyCode::Char('q')) => {
            if app.has_active_execution() && key.code == KeyCode::Char('c') {
                app.abort_current_execution();
            } else {
                app.should_quit = true;
            }
        }

        // ── Switch to Insert Mode ──
        (_, KeyCode::Char('i')) | (_, KeyCode::Enter) => {
            app.set_mode(AppMode::Insert);
        }

        // ── Sidebar Toggle ──
        (_, KeyCode::Tab) => {
            app.toggle_sidebar();
            // If opening sidebar, focus it (optional, currently toggle logic handles focus reset)
            if app.ui_state.show_sidebar {
                app.ui_state.focus = Focus::Sidebar;
            } else {
                app.ui_state.focus = Focus::Chat;
            }
        }

        // ── Vim Navigation ──
        (_, KeyCode::Char('j')) | (_, KeyCode::Down) => {
            app.scroll_down();
        }
        (_, KeyCode::Char('k')) | (_, KeyCode::Up) => {
            app.scroll_up();
        }
        (_, KeyCode::Char('G')) | (KeyModifiers::SHIFT, KeyCode::Char('g')) => {
            // Scroll to bottom (G)
            app.scroll_to_bottom();
        }
        (_, KeyCode::Char('g')) => {
            // Simple 'g' for top (vim is gg)
            app.ui_state.scroll_offset = usize::MAX;
        }

        // Emulate 'gg' via Home for now, or just mapping 'g' to top if modifier not checked carefully
        (_, KeyCode::Home) => {
            // Scroll to top: we don't have a direct "top" fn yet, but usually offset = huge
            // We need a way to know max scroll.
            // Ui renders and calculates max_scroll. App doesn't know layout height easily.
            // But we can just set offset to usize::MAX? No, offset is from bottom usually?
            // In `ui.rs`: `max_scroll - app.ui_state.scroll_offset`.
            // If we set `scroll_offset` to `usize::MAX`, it triggers saturating sub.
            // Let's verify `ui.rs`:
            // `let scroll_pos = max_scroll.saturating_sub(app.ui_state.scroll_offset as u16);`
            // If offset is huge, scroll_pos is 0 (Top).
            // Wait. `scroll_pos` 0 is TOP?
            // `paragraph.scroll((scroll_pos, 0))`
            // Usually 0 is top.
            // If `max_scroll` is say 100 (lines).
            // We want bottom view. `scroll_pos` should be max_scroll?
            // If `offset` is 0 (default): `scroll_pos` = `max_scroll`. -> Scrolled to bottom?
            // Ratatui Paragraph scroll is `(offset_y, offset_x)`. 0 means top line.
            // So if we want bottom, we need `offset_y` to be `total_lines - height`.

            // App `scroll_offset`: 0 = "Show Bottom".
            // `ui.rs` logic: `max_scroll - scroll_offset`.
            // If offset = 0, we render from `max_scroll` line?
            // Let's check `ui.rs` again logic.
            // `let max_scroll = total_wrapped.saturating_sub(inner.height);`
            // `let scroll_pos = max_scroll.saturating_sub(app.ui_state.scroll_offset as u16);`
            // If offset=0 -> `scroll_pos = max_scroll`. Paragraph scroll set to `max_scroll`. -> Bottom. Correct.
            // To scroll to TOP (`scroll_pos = 0`), we need `scroll_offset = max_scroll`.
            // Since we don't know `max_scroll` in `App`, we can just ensure `scroll_offset` is large enough.
            app.ui_state.scroll_offset = usize::MAX;
        }
        (_, KeyCode::End) => {
            app.scroll_to_bottom();
        }

        (_, KeyCode::PageUp) | (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
            for _ in 0..10 {
                app.scroll_up();
            }
        }
        (_, KeyCode::PageDown) | (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            for _ in 0..10 {
                app.scroll_down();
            }
        }

        // ── Screen / UI ────────────────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => {
            app.messages.clear();
            app.scroll_to_bottom();
        }
        (_, KeyCode::F(2)) => {
            app.ui_state.mouse_captured = !app.ui_state.mouse_captured;
            if app.ui_state.mouse_captured {
                execute!(std::io::stdout(), crossterm::event::EnableMouseCapture).ok();
            } else {
                execute!(std::io::stdout(), crossterm::event::DisableMouseCapture).ok();
            }
        }

        _ => {}
    }
}

fn handle_command_mode(app: &mut App, key: KeyEvent) {
    // Escape to return to Normal
    if let KeyCode::Esc = key.code {
        app.set_mode(AppMode::Normal);
    }
    // TODO: Implement command entry
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            for _ in 0..MOUSE_SCROLL_LINES {
                app.scroll_up();
            }
        }
        MouseEventKind::ScrollDown => {
            for _ in 0..MOUSE_SCROLL_LINES {
                app.scroll_down();
            }
        }
        MouseEventKind::Down(_) => {
            // Basic click to focus not fully implemented with regions,
            // but could check coordinates. For now, click anywhere likely captures mouse.
        }
        _ => {}
    }
}
