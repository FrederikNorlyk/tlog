use crate::tui::components::dialog::Dialog;
use crate::tui::components::keybinds_dialog::KeybindDialogEvent::{Closed, Consumed};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget};
use ratatui_textarea::TextArea;

pub struct KeybindsDialog<'a> {
    text_area: TextArea<'a>,
    state: ListState,
    all_keybinds: Vec<Keybind>,
    filtered_keybinds: Vec<Keybind>,
    is_text_area_focused: bool,
}

impl KeybindsDialog<'_> {
    #[must_use]
    pub fn new(keybinds: Vec<Keybind>) -> Self {
        let mut text_area = TextArea::new(vec![String::new()]);
        text_area.set_style(Style::default().fg(Color::DarkGray));
        text_area.set_placeholder_text("/ to search");
        text_area.set_cursor_style(Style::default());

        text_area.set_block(
            Block::default()
                .border_style(Color::DarkGray)
                .borders(Borders::BOTTOM),
        );

        let mut state = ListState::default();
        state.select_first();

        Self {
            text_area,
            state,
            all_keybinds: keybinds.clone(),
            filtered_keybinds: keybinds,
            is_text_area_focused: false,
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> KeybindDialogEvent {
        let ctrl_key_is_held = key_event.modifiers.contains(KeyModifiers::CONTROL);

        match key_event.code {
            KeyCode::Down => {
                self.state.select_previous();
                if self.is_text_area_focused {
                    self.toggle_focused_field();
                }
            }
            KeyCode::Up => {
                self.state.select_next();
                if self.is_text_area_focused {
                    self.toggle_focused_field();
                }
            }
            KeyCode::Char('j') if ctrl_key_is_held || !self.is_text_area_focused => {
                self.state.select_previous();
                if self.is_text_area_focused {
                    self.toggle_focused_field();
                }
            }
            KeyCode::Char('k') if ctrl_key_is_held || !self.is_text_area_focused => {
                self.state.select_next();
                if self.is_text_area_focused {
                    self.toggle_focused_field();
                }
            }
            KeyCode::Char('/') if !self.is_text_area_focused => {
                self.state = ListState::default();
                self.toggle_focused_field();
            }
            KeyCode::Esc => {
                return Closed;
            }
            _ => {
                if self.is_text_area_focused {
                    self.text_area.input(key_event);
                    self.search_keybinds();
                }
            }
        }

        Consumed
    }

    fn toggle_focused_field(&mut self) {
        self.is_text_area_focused = !self.is_text_area_focused;

        if self.is_text_area_focused {
            self.text_area
                .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            self.text_area.set_cursor_style(Style::default());
        }
    }

    fn search_keybinds(&mut self) {
        let query = self.text_area.lines().first().map_or("", String::as_str);

        self.filtered_keybinds = self
            .all_keybinds
            .iter()
            .filter(|k| k.key.contains(query) || k.description.contains(query))
            .cloned()
            .collect();
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let dialog = Dialog::constrained(Constraint::Percentage(90), Constraint::Percentage(70));
        let inner = dialog.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Fill(1)])
            .split(inner);

        let items = self.filtered_keybinds.iter().map(|keybind| {
            let key = keybind.key.clone();
            let description = keybind.description.clone();
            ListItem::new(Text::from(format!("{key} - {description}")))
        });

        let list = List::new(items).highlight_style(Style::new().reversed());

        self.text_area.render(chunks[0], buf);

        StatefulWidget::render(list, chunks[1], buf, &mut self.state);
    }
}

pub enum KeybindDialogEvent {
    Closed,
    Consumed,
}

#[derive(Clone)]
pub struct Keybind {
    key: String,
    description: String,
}

impl Keybind {
    #[must_use]
    pub fn new(key: String, description: String) -> Self {
        Self { key, description }
    }
}
