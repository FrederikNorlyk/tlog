use crate::tui::components::dialog::Dialog;
use crate::tui::components::keybinds_dialog::KeybindDialogEvent::{Closed, Consumed};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget};
use ratatui_textarea::TextArea;
use std::fmt::{Display, Formatter};

pub struct KeybindsDialog<'a> {
    text_area: TextArea<'a>,
    list_state: ListState,
    all_keybinds: Vec<Keybind>,
    filtered_keybinds: Vec<Keybind>,
    is_text_area_focused: bool,
    list_height: u16,
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

        let mut list_state = ListState::default();
        list_state.select_first();

        Self {
            text_area,
            list_state,
            all_keybinds: keybinds.clone(),
            filtered_keybinds: keybinds,
            is_text_area_focused: false,
            list_height: 0,
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> KeybindDialogEvent {
        let half_page = self.list_height.saturating_sub(1) / 2;
        let ctrl_key_is_held = key_event.modifiers.contains(KeyModifiers::CONTROL);

        match key_event.code {
            KeyCode::Down => {
                self.list_state.select_next();
                self.unfocus_textarea();
            }
            KeyCode::Up => {
                self.list_state.select_previous();
                self.unfocus_textarea();
            }
            KeyCode::Char('j') if ctrl_key_is_held || !self.is_text_area_focused => {
                self.list_state.select_next();
                self.unfocus_textarea();
            }
            KeyCode::Char('k') if ctrl_key_is_held || !self.is_text_area_focused => {
                self.list_state.select_previous();
                self.unfocus_textarea();
            }
            KeyCode::Char('g') if !self.is_text_area_focused => {
                self.list_state.select_first();
                self.unfocus_textarea();
            }
            KeyCode::Home => {
                self.list_state.select_first();
                self.unfocus_textarea();
            }
            KeyCode::Char('G') if !self.is_text_area_focused => {
                self.list_state.select_last();
                self.unfocus_textarea();
            }
            KeyCode::End => {
                self.list_state.select_last();
                self.unfocus_textarea();
            }
            KeyCode::Char('u') if ctrl_key_is_held => {
                self.list_state.scroll_up_by(half_page);
                self.unfocus_textarea();
            }
            KeyCode::Char('d') if ctrl_key_is_held => {
                self.list_state.scroll_down_by(half_page);
                self.unfocus_textarea();
            }
            KeyCode::Char('/') if !self.is_text_area_focused => {
                self.list_state = ListState::default();
                self.toggle_textarea_focus();
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

    fn unfocus_textarea(&mut self) {
        if self.is_text_area_focused {
            self.toggle_textarea_focus();
        }
    }

    fn toggle_textarea_focus(&mut self) {
        self.is_text_area_focused = !self.is_text_area_focused;

        if self.is_text_area_focused {
            self.text_area
                .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            self.text_area.set_cursor_style(Style::default());
        }
    }

    fn search_keybinds(&mut self) {
        let query: String = self
            .text_area
            .lines()
            .first()
            .map_or("", String::as_str)
            .to_lowercase();

        self.filtered_keybinds = self
            .all_keybinds
            .iter()
            .filter(|k| {
                k.key.to_lowercase().contains(&query)
                    || k.description.to_lowercase().contains(&query)
            })
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

        let list_area = chunks[1];
        self.list_height = list_area.height;

        StatefulWidget::render(list, list_area, buf, &mut self.list_state);
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum KeybindDialogEvent {
    Closed,
    Consumed,
}

#[derive(Clone)]
pub struct Keybind {
    key: String,
    description: String,
}

impl Display for Keybind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.key, self.description)
    }
}

impl Keybind {
    #[must_use]
    pub fn new(key: String, description: String) -> Self {
        Self { key, description }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    mod handle_key_event {
        use super::*;
        use crate::tui::components::session_table::SessionTable;

        #[test]
        fn navigation() {
            let mut dialog = KeybindsDialog::new(SessionTable::get_keybinds());
            dialog.list_height = 10;

            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 0);

            let event = dialog.handle_key_event(key(KeyCode::Char('j')));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 1);

            let event = dialog.handle_key_event(key(KeyCode::Down));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 2);

            let event = dialog.handle_key_event(key(KeyCode::Char('k')));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 1);

            let event = dialog.handle_key_event(key(KeyCode::Up));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 0);

            // '/' moves focus to text area
            let event = dialog.handle_key_event(key(KeyCode::Char('/')));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected();
            assert!(selected_index.is_none());

            // Typing 'j' is consumed by the text area
            let event = dialog.handle_key_event(key(KeyCode::Char('j')));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected();
            assert!(selected_index.is_none());

            // Typing 'k' is consumed by the text area
            let event = dialog.handle_key_event(key(KeyCode::Char('k')));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected();
            assert!(selected_index.is_none());

            // Typing 'ctrl+j' jumps out of the text area
            let key_event = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
            let event = dialog.handle_key_event(key_event);
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 0);

            // '/' moves focus to text area
            let event = dialog.handle_key_event(key(KeyCode::Char('/')));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected();
            assert!(selected_index.is_none());

            // Typing 'ctrl+k' jumps out of the text area (selects largest index)
            let key_event = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
            let event = dialog.handle_key_event(key_event);
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 18_446_744_073_709_551_615);

            // Typing 'g' selects first index
            let event = dialog.handle_key_event(key(KeyCode::Char('g')));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 0);

            // Typing 'G' selects last index
            let event = dialog.handle_key_event(key(KeyCode::Char('G')));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 18_446_744_073_709_551_615);

            // Home key selects first index
            let event = dialog.handle_key_event(key(KeyCode::Home));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 0);

            // End key selects last index
            let event = dialog.handle_key_event(key(KeyCode::End));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 18_446_744_073_709_551_615);

            // Home key back to index 0
            let event = dialog.handle_key_event(key(KeyCode::Home));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 0);

            // Typing 'ctrl+d' scrolls down half a page
            let key_event = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
            let event = dialog.handle_key_event(key_event);
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 4);

            // Typing 'ctrl+d' scrolls down half a page
            let key_event = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
            let event = dialog.handle_key_event(key_event);
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 8);

            // Typing 'ctrl+u' scrolls up half a page
            let key_event = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
            let event = dialog.handle_key_event(key_event);
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected().unwrap();
            assert_eq!(selected_index, 4);
        }

        #[test]
        fn search() {
            let mut dialog = KeybindsDialog::new(SessionTable::get_keybinds());

            // '/' moves focus to text area
            let event = dialog.handle_key_event(key(KeyCode::Char('/')));
            assert_eq!(event, Consumed);
            let selected_index = dialog.list_state.selected();
            assert!(selected_index.is_none());

            // Type 'page'
            dialog.handle_key_event(key(KeyCode::Char('p')));
            dialog.handle_key_event(key(KeyCode::Char('a')));
            dialog.handle_key_event(key(KeyCode::Char('g')));
            dialog.handle_key_event(key(KeyCode::Char('e')));
            assert_eq!(dialog.filtered_keybinds.len(), 4);

            let first = dialog.filtered_keybinds.first().unwrap();
            let second = dialog.filtered_keybinds.get(1).unwrap();
            let third = dialog.filtered_keybinds.get(2).unwrap();
            let fourth = dialog.filtered_keybinds.get(3).unwrap();

            assert_eq!(format!("{first}"), "ctrl+u - Scroll up half a page");
            assert_eq!(format!("{second}"), "ctrl+d - Scroll down half a page");
            assert_eq!(format!("{third}"), "page up - Scroll up a page");
            assert_eq!(format!("{fourth}"), "page down - Scroll down a page");
        }

        #[test]
        fn close() {
            let mut dialog = KeybindsDialog::new(SessionTable::get_keybinds());

            let event = dialog.handle_key_event(key(KeyCode::Esc));
            assert_eq!(event, Closed);
        }
    }

    mod render {
        use super::*;
        use crate::tui::components::project_table::ProjectTable;
        use crate::tui::components::session_table::SessionTable;
        use crate::tui::render_test_util::RenderTestUtil;

        // TODO: Focused text field. Filtered rows
        #[test]
        fn session_table() {
            let mut dialog = KeybindsDialog::new(SessionTable::get_keybinds());

            let area = Rect::new(0, 0, 40, 30);
            let mut buf = Buffer::empty(area);

            dialog.render(area, &mut buf);

            let expected = vec![
                "                                        ",
                "                                        ",
                "                                        ",
                "                                        ",
                "                                        ",
                "  ┌───────────────────────────── Esc ┐  ",
                "  │ / to search                      │  ",
                "  │──────────────────────────────────│  ",
                "  │a - Track a new project           │  ",
                "  │e - Edit tracked time             │  ",
                "  │space - Toggle time tracking      │  ",
                "  │d - Delete session                │  ",
                "  │ctrl+a - Increment session by 15 m│  ",
                "  │ctrl+x - Decrement session by 15 m│  ",
                "  │delete - Delete session           │  ",
                "  │D - Force delete session          │  ",
                "  │c - Copy                          │  ",
                "  │f - Change time format            │  ",
                "  │k - Select previous row           │  ",
                "  │↑ - Select previous row           │  ",
                "  │j - Select next row               │  ",
                "  │↓ - Select next row               │  ",
                "  │g - Select first row              │  ",
                "  │home - Select first row           │  ",
                "  │G - Select last row               │  ",
                "  └──────────────────────────────────┘  ",
                "                                        ",
                "                                        ",
                "                                        ",
                "                                        ",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn project_table() {
            let mut dialog = KeybindsDialog::new(ProjectTable::get_keybinds());

            let area = Rect::new(0, 0, 40, 30);
            let mut buf = Buffer::empty(area);

            dialog.render(area, &mut buf);

            let expected = vec![
                "                                        ",
                "                                        ",
                "                                        ",
                "                                        ",
                "                                        ",
                "  ┌───────────────────────────── Esc ┐  ",
                "  │ / to search                      │  ",
                "  │──────────────────────────────────│  ",
                "  │a - Add a new project             │  ",
                "  │e - Edit project                  │  ",
                "  │d - Delete project                │  ",
                "  │delete - Delete project           │  ",
                "  │D - Force delete project          │  ",
                "  │k - Select previous row           │  ",
                "  │↑ - Select previous row           │  ",
                "  │j - Select next row               │  ",
                "  │↓ - Select next row               │  ",
                "  │g - Select first row              │  ",
                "  │home - Select first row           │  ",
                "  │G - Select last row               │  ",
                "  │end - Select last row             │  ",
                "  │ctrl+u - Scroll up half a page    │  ",
                "  │ctrl+d - Scroll down half a page  │  ",
                "  │page up - Scroll up a page        │  ",
                "  │page down - Scroll down a page    │  ",
                "  └──────────────────────────────────┘  ",
                "                                        ",
                "                                        ",
                "                                        ",
                "                                        ",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn searching() {
            let mut dialog = KeybindsDialog::new(ProjectTable::get_keybinds());
            dialog.handle_key_event(key(KeyCode::Char('/')));
            dialog.handle_key_event(key(KeyCode::Char('d')));
            dialog.handle_key_event(key(KeyCode::Char('e')));
            dialog.handle_key_event(key(KeyCode::Char('l')));
            dialog.handle_key_event(key(KeyCode::Char('e')));
            dialog.handle_key_event(key(KeyCode::Char('t')));
            dialog.handle_key_event(key(KeyCode::Char('e')));

            let area = Rect::new(0, 0, 40, 30);
            let mut buf = Buffer::empty(area);

            dialog.render(area, &mut buf);

            let expected = vec![
                "                                        ",
                "                                        ",
                "                                        ",
                "                                        ",
                "                                        ",
                "  ┌───────────────────────────── Esc ┐  ",
                "  │delete                            │  ",
                "  │──────────────────────────────────│  ",
                "  │d - Delete project                │  ",
                "  │delete - Delete project           │  ",
                "  │D - Force delete project          │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  │                                  │  ",
                "  └──────────────────────────────────┘  ",
                "                                        ",
                "                                        ",
                "                                        ",
                "                                        ",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }
    }
}
