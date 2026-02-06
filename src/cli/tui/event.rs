//! Crossterm event handling for the TUI

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

use super::app::App;

/// Poll crossterm events and update app state.
/// Returns `true` if the app should quit.
pub fn handle_events(app: &mut App, timeout: Duration) -> Result<()> {
    // Drain any pending LLM responses first.
    app.poll_responses();

    if event::poll(timeout)? {
        if let Event::Key(key) = event::read()? {
            handle_key(app, key);
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
            app.scroll_offset = 0;
        }
        (_, KeyCode::F(1)) => app.toggle_sidebar(),

        // ── Scroll chat ────────────────────────────────────────
        (_, KeyCode::Up) => app.scroll_up(),
        (_, KeyCode::Down) => app.scroll_down(),
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

        // ── Submit (blocked during loading) ────────────────────
        (_, KeyCode::Enter) => {
            if !app.is_loading {
                app.submit();
            }
        }

        // ── Line editing (Emacs-style) ─────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('a')) => app.move_cursor_home(),
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => app.move_cursor_end(),
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => app.clear_to_start(),
        (KeyModifiers::CONTROL, KeyCode::Char('k')) => app.clear_to_end(),
        (KeyModifiers::CONTROL, KeyCode::Char('w')) => app.delete_word_before_cursor(),

        // ── Word navigation (Alt+Arrow) ────────────────────────
        (KeyModifiers::ALT, KeyCode::Left) => app.move_cursor_word_left(),
        (KeyModifiers::ALT, KeyCode::Right) => app.move_cursor_word_right(),
        // Alt+Backspace = delete word backward
        (KeyModifiers::ALT, KeyCode::Backspace) => app.delete_word_before_cursor(),

        // ── Cursor movement ────────────────────────────────────
        (_, KeyCode::Left) => app.move_cursor_left(),
        (_, KeyCode::Right) => app.move_cursor_right(),
        (_, KeyCode::Home) => app.move_cursor_home(),
        (_, KeyCode::End) => app.move_cursor_end(),
        (_, KeyCode::Backspace) => app.delete_char_before_cursor(),
        (_, KeyCode::Delete) => app.delete_char_after_cursor(),

        // ── Character input (allowed while loading) ────────────
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            app.insert_char(c);
        }

        _ => {}
    }
}
