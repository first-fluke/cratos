//! TUI Settings Modal
//!
//! Renders an overlay settings modal when Focus::Settings is active.
//! Categories on the left, fields on the right. Edit values with Enter/Esc.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

/// A single setting field displayed in the modal
pub struct SettingField {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
}

/// Per-category settings data
pub struct SettingsCategory {
    pub name: &'static str,
    pub fields: Vec<SettingField>,
}

/// Settings modal state
pub struct SettingsState {
    pub category_index: usize,
    pub field_index: usize,
    pub editing: bool,
    pub edit_buffer: String,
    pub categories: Vec<SettingsCategory>,
    pub category_list_state: ListState,
    pub field_list_state: ListState,
}

impl SettingsState {
    pub fn load() -> Self {
        let config = crate::server::load_config().unwrap_or_default();
        let view = crate::api::config::AppConfigView::from(config);

        let categories = vec![
            SettingsCategory {
                name: "General",
                fields: vec![
                    SettingField {
                        key: "general.language",
                        label: "Language",
                        value: view.general.language,
                    },
                    SettingField {
                        key: "general.persona",
                        label: "Persona",
                        value: view.general.persona,
                    },
                ],
            },
            SettingsCategory {
                name: "LLM",
                fields: vec![
                    SettingField {
                        key: "llm.default_provider",
                        label: "Provider",
                        value: view.llm.default_provider,
                    },
                    SettingField {
                        key: "llm.model",
                        label: "Model",
                        value: view.llm.model,
                    },
                ],
            },
            SettingsCategory {
                name: "Channels",
                fields: vec![
                    SettingField {
                        key: "channels.telegram_enabled",
                        label: "Telegram",
                        value: view.channels.telegram_enabled.to_string(),
                    },
                    SettingField {
                        key: "channels.slack_enabled",
                        label: "Slack",
                        value: view.channels.slack_enabled.to_string(),
                    },
                    SettingField {
                        key: "channels.discord_enabled",
                        label: "Discord",
                        value: view.channels.discord_enabled.to_string(),
                    },
                ],
            },
            SettingsCategory {
                name: "Security",
                fields: vec![
                    SettingField {
                        key: "security.approval_mode",
                        label: "Approval Mode",
                        value: view.security.approval_mode,
                    },
                    SettingField {
                        key: "security.exec_mode",
                        label: "Exec Mode",
                        value: view.security.exec_mode,
                    },
                    SettingField {
                        key: "security.injection_protection",
                        label: "Injection Protection",
                        value: view.security.injection_protection.to_string(),
                    },
                ],
            },
            SettingsCategory {
                name: "Tools",
                fields: vec![
                    SettingField {
                        key: "tools.scheduler_enabled",
                        label: "Scheduler",
                        value: view.tools.scheduler_enabled.to_string(),
                    },
                    SettingField {
                        key: "tools.vector_search_enabled",
                        label: "Vector Search",
                        value: view.tools.vector_search_enabled.to_string(),
                    },
                ],
            },
            SettingsCategory {
                name: "Advanced",
                fields: vec![
                    SettingField {
                        key: "advanced.server_port",
                        label: "Server Port",
                        value: view.advanced.server_port.to_string(),
                    },
                    SettingField {
                        key: "advanced.replay_retention_days",
                        label: "Replay Retention (days)",
                        value: view.advanced.replay_retention_days.to_string(),
                    },
                ],
            },
        ];

        let mut cat_state = ListState::default();
        cat_state.select(Some(0));
        let mut field_state = ListState::default();
        field_state.select(Some(0));

        Self {
            category_index: 0,
            field_index: 0,
            editing: false,
            edit_buffer: String::new(),
            categories,
            category_list_state: cat_state,
            field_list_state: field_state,
        }
    }

    pub fn current_category(&self) -> &SettingsCategory {
        &self.categories[self.category_index]
    }

    pub fn move_category(&mut self, delta: i32) {
        let len = self.categories.len() as i32;
        let new = (self.category_index as i32 + delta).rem_euclid(len) as usize;
        self.category_index = new;
        self.category_list_state.select(Some(new));
        self.field_index = 0;
        self.field_list_state.select(Some(0));
    }

    pub fn move_field(&mut self, delta: i32) {
        let len = self.current_category().fields.len() as i32;
        if len == 0 {
            return;
        }
        let new = (self.field_index as i32 + delta).rem_euclid(len) as usize;
        self.field_index = new;
        self.field_list_state.select(Some(new));
    }

    pub fn start_edit(&mut self) {
        let field = &self.current_category().fields[self.field_index];
        self.edit_buffer = field.value.clone();
        self.editing = true;
    }

    pub fn cancel_edit(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
    }

    pub fn confirm_edit(&mut self) -> Option<(&'static str, String)> {
        self.editing = false;
        let key = self.categories[self.category_index].fields[self.field_index].key;
        let value = self.edit_buffer.clone();
        // Update the display value
        self.categories[self.category_index].fields[self.field_index].value = value.clone();
        self.edit_buffer.clear();
        Some((key, value))
    }
}

/// Draw the settings modal as an overlay
pub fn draw_settings(frame: &mut Frame, state: &mut SettingsState) {
    let area = frame.area();

    // Center the modal (80% of screen, min 60x20)
    let modal = centered_rect(80, 80, area);

    // Clear the area behind the modal
    frame.render_widget(Clear, modal);

    let block = Block::default()
        .title(" Settings (F5 to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    // Split into left (categories) and right (fields)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(20), Constraint::Min(30)])
        .split(inner);

    // Left: category list
    let cat_items: Vec<ListItem> = state
        .categories
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let style = if i == state.category_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!(" {} ", cat.name)).style(style)
        })
        .collect();
    let cat_list = List::new(cat_items)
        .block(
            Block::default()
                .title(" Categories ")
                .borders(Borders::RIGHT),
        )
        .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black));
    frame.render_stateful_widget(cat_list, chunks[0], &mut state.category_list_state);

    // Right: field list
    let cat = &state.categories[state.category_index];
    let field_items: Vec<ListItem> = cat
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let is_selected = i == state.field_index;
            let is_editing = is_selected && state.editing;

            let display_value = if is_editing {
                format!("{}|", state.edit_buffer)
            } else {
                field.value.clone()
            };

            let style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<22} ", field.label), style),
                Span::styled(
                    display_value,
                    if is_editing {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ),
            ]))
        })
        .collect();

    let field_list = List::new(field_items).block(Block::default().title(format!(
        " {} ",
        state.categories[state.category_index].name
    )));
    frame.render_stateful_widget(field_list, chunks[1], &mut state.field_list_state);

    // Help text at the bottom
    let help_text = if state.editing {
        "Type value, Enter to confirm, Esc to cancel"
    } else {
        "j/k: move | h/l: category | Enter: edit | s: save | Esc/F5: close"
    };
    let help_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true }),
        help_area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
