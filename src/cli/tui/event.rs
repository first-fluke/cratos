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
        // Quit shortcuts
        (KeyModifiers::CONTROL, KeyCode::Char('c')) | (_, KeyCode::Esc) => {
            app.should_quit = true;
        }
        // Clear screen
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => {
            app.messages.clear();
            app.scroll_offset = 0;
        }
        // Submit message
        (_, KeyCode::Enter) => {
            if !app.is_loading {
                app.submit();
            }
        }
        // Cursor movement
        (_, KeyCode::Left) => app.move_cursor_left(),
        (_, KeyCode::Right) => app.move_cursor_right(),
        (_, KeyCode::Backspace) => app.delete_char_before_cursor(),
        // Scroll chat
        (_, KeyCode::Up) => app.scroll_up(),
        (_, KeyCode::Down) => app.scroll_down(),
        // Home / End
        (_, KeyCode::Home) => app.cursor_pos = 0,
        (_, KeyCode::End) => app.cursor_pos = app.input.len(),
        // Character input
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            if !app.is_loading {
                app.insert_char(c);
            }
        }
        _ => {}
    }
}
