//! Crossterm event handling for the TUI

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use std::time::Duration;

use super::app::App;

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
    match (key.modifiers, key.code) {
        // ── Quit ────────────────────────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('c')) | (_, KeyCode::Esc) => {
            app.should_quit = true;
        }

        // ── Screen / UI ────────────────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => {
            app.messages.clear();
            app.ui_state.scroll_offset = 0;
        }
        (_, KeyCode::F(1)) => app.toggle_sidebar(),
        (_, KeyCode::F(2)) => {
            app.ui_state.mouse_captured = !app.ui_state.mouse_captured;
            if app.ui_state.mouse_captured {
                execute!(std::io::stdout(), EnableMouseCapture).ok();
            } else {
                execute!(std::io::stdout(), DisableMouseCapture).ok();
            }
        }

        // ── Scroll / History (Up/Down depend on input state) ──
        (_, KeyCode::Up) if app.is_input_empty() => {
            if app.has_history() {
                app.history_up();
            } else {
                app.scroll_up();
            }
        }
        (_, KeyCode::Down) if app.is_input_empty() => {
            app.scroll_down();
        }
        (_, KeyCode::Up) => app.history_up(),
        (_, KeyCode::Down) => app.history_down(),
        (_, KeyCode::PageUp) => {
            for _ in 0..10 {
                app.scroll_up();
            }
        }
        (_, KeyCode::PageDown) => {
            for _ in 0..10 {
                app.scroll_down();
            }
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

        // ── Submit (blocked during loading) ────────────────────
        (_, KeyCode::Enter) => {
            if !app.ui_state.is_loading {
                app.submit();
            }
        }

        // ── Delegate everything else to textarea ────────────────
        _ => {
            app.textarea.input(Event::Key(key));
            app.update_suggestions();
        }
    }
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
        _ => {}
    }
}
