//! TUI slash command registry and handler

use crate::cli::tui::app::App;
use anyhow::Result;

/// A slash command definition.
pub struct Command {
    pub name: &'static str,
    pub handler: fn(&mut App, &[&str]) -> Result<()>,
}

pub struct CommandRegistry {
    commands: Vec<Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            commands: Vec::new(),
        };
        registry.register_defaults();
        registry
    }

    fn register_defaults(&mut self) {
        self.commands.push(Command {
            name: "help",
            handler: |app, _| {
                let mut help_text = String::from("Available commands:\n");
                help_text.push_str("  /persona <name>  Switch persona\n");
                help_text.push_str("  /clear           Clear chat\n");
                help_text.push_str("  /help            Show this help\n");
                help_text.push_str("  /quit            Exit TUI\n");

                app.push_system(help_text);
                Ok(())
            },
        });

        self.commands.push(Command {
            name: "clear",
            handler: |app, _| {
                app.messages.clear();
                app.ui_state.scroll_offset = 0;
                app.push_system("Chat cleared.".into());
                Ok(())
            },
        });

        self.commands.push(Command {
            name: "quit",
            handler: |app, _| {
                app.should_quit = true;
                Ok(())
            },
        });

        self.commands.push(Command {
            name: "persona",
            handler: |app, args| {
                if let Some(name) = args.first() {
                    app.persona = name.to_string();
                    app.push_system(format!("Persona switched to: {}", app.persona));
                } else {
                    app.push_system(format!("Current persona: {}", app.persona));
                }
                Ok(())
            },
        });

        self.commands.push(Command {
            name: "abort",
            handler: |app, _| {
                app.abort_execution();
                Ok(())
            },
        });
    }

    pub fn handle(&self, app: &mut App, input: &str) -> Result<()> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        let cmd_name = parts[0].strip_prefix('/').unwrap_or(parts[0]);
        let args = &parts[1..];

        if let Some(cmd) = self.commands.iter().find(|c| c.name == cmd_name) {
            (cmd.handler)(app, args)?;
        } else {
            app.push_system(format!("Unknown command: /{}", cmd_name));
        }

        Ok(())
    }

    pub fn get_suggestions(&self, input: &str) -> Vec<&'static str> {
        if !input.starts_with('/') {
            return Vec::new();
        }
        let search = input.strip_prefix('/').unwrap_or("");
        self.commands
            .iter()
            .filter(|c| c.name.starts_with(search))
            .map(|c| c.name)
            .collect()
    }
}
