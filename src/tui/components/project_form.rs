use crate::tui::components::dialog::Dialog;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Color, Modifier, Widget};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders};
use ratatui_textarea::TextArea;

pub struct ProjectForm<'a> {
    id: Option<i32>,
    name_text_area: TextArea<'a>,
    description_text_area: TextArea<'a>,
    is_name_focused: bool,
}

impl ProjectForm<'_> {
    #[must_use]
    pub fn new(id: Option<i32>, name: Option<String>, description: Option<String>) -> Self {
        let mut name_text_area = TextArea::new(vec![name.unwrap_or_default()]);
        name_text_area.set_style(Style::default().fg(Color::DarkGray));

        name_text_area.set_block(
            Block::default()
                .border_style(Color::DarkGray)
                .borders(Borders::ALL)
                .title("Name"),
        );

        let mut description_text_area = TextArea::new(vec![description.unwrap_or_default()]);

        description_text_area.set_style(Style::default().fg(Color::DarkGray));

        description_text_area.set_block(
            Block::default()
                .border_style(Color::DarkGray)
                .borders(Borders::ALL)
                .title("Description"),
        );

        description_text_area.set_cursor_style(Style::default());

        Self {
            id,
            name_text_area,
            description_text_area,
            is_name_focused: true,
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> ProjectFormEvent {
        match key_event.code {
            KeyCode::Esc => return ProjectFormEvent::Cancel,
            KeyCode::Tab => self.toggle_focused_field(),
            KeyCode::Enter => {
                let is_valid = self.validate_form();
                if is_valid {
                    return if let Some(id) = self.id {
                        ProjectFormEvent::Update {
                            id,
                            name: self.get_name_value(),
                            description: self.get_description_value(),
                        }
                    } else {
                        ProjectFormEvent::Create {
                            name: self.get_name_value(),
                            description: self.get_description_value(),
                        }
                    };
                }
            }
            _ => {
                if self.is_name_focused {
                    self.name_text_area.input(key_event);
                } else {
                    self.description_text_area.input(key_event);
                }
            }
        }

        ProjectFormEvent::Consumed
    }

    fn validate_form(&mut self) -> bool {
        let mut is_valid = true;

        let Some(block) = self.name_text_area.block().cloned() else {
            return false;
        };

        if self.get_name_value().is_empty() {
            is_valid = false;
            self.name_text_area
                .set_block(block.border_style(Color::Red));
        } else {
            self.name_text_area
                .set_block(block.border_style(Color::DarkGray));
        }

        is_valid
    }

    fn get_name_value(&self) -> String {
        self.name_text_area
            .lines()
            .first()
            .map_or("", String::as_str)
            .trim()
            .to_string()
    }

    fn get_description_value(&self) -> Option<String> {
        let description = self
            .description_text_area
            .lines()
            .first()
            .map_or("", String::as_str)
            .trim()
            .to_string();

        if description.is_empty() {
            None
        } else {
            Some(description)
        }
    }

    fn toggle_focused_field(&mut self) {
        self.is_name_focused = !self.is_name_focused;

        if self.is_name_focused {
            self.name_text_area
                .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            self.description_text_area
                .set_cursor_style(Style::default());
        } else {
            self.description_text_area
                .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            self.name_text_area.set_cursor_style(Style::default());
        }
    }
}

impl Widget for &ProjectForm<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog = Dialog::constrained(Constraint::Percentage(90), Constraint::Length(10));
        let inner = dialog.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Fill(1)])
            .split(inner);

        self.name_text_area.render(chunks[0], buf);
        self.description_text_area.render(chunks[1], buf);
    }
}

pub enum ProjectFormEvent {
    Create {
        name: String,
        description: Option<String>,
    },
    Update {
        id: i32,
        name: String,
        description: Option<String>,
    },
    Cancel,
    Consumed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    mod handle_key_event {
        use super::*;

        #[test]
        fn esc_returns_cancel() {
            let mut form = ProjectForm::new(None, None, None);

            let event = form.handle_key_event(key(KeyCode::Esc));

            match event {
                ProjectFormEvent::Cancel => {}
                _ => panic!("expected Cancel"),
            }
        }

        #[test]
        fn tab_toggles_focus_and_consumes() {
            let mut form = ProjectForm::new(None, None, None);

            let event = form.handle_key_event(key(KeyCode::Tab));

            match event {
                ProjectFormEvent::Consumed => {}
                _ => panic!("expected Consumed"),
            }

            assert!(!form.is_name_focused);
        }

        #[test]
        fn enter_with_valid_name_returns_save() {
            let mut form = ProjectForm::new(None, Some("Project".into()), Some("desc".into()));

            let event = form.handle_key_event(key(KeyCode::Enter));

            match event {
                ProjectFormEvent::Create { name, description } => {
                    assert_eq!(name, "Project");
                    assert_eq!(description, Some("desc".to_string()));
                }
                _ => panic!("expected Save"),
            }
        }

        #[test]
        fn enter_with_invalid_name_returns_consumed_and_marks_invalid() {
            let mut form = ProjectForm::new(None, None, None);

            let event = form.handle_key_event(key(KeyCode::Enter));

            match event {
                ProjectFormEvent::Consumed => {}
                _ => panic!("expected Consumed"),
            }
        }

        #[test]
        fn typing_goes_to_name_field_when_focused() {
            let mut form = ProjectForm::new(None, None, None);

            form.handle_key_event(key(KeyCode::Char('a')));

            assert_eq!(form.get_name_value(), "a");
        }

        #[test]
        fn typing_goes_to_description_when_not_focused() {
            let mut form = ProjectForm::new(None, None, None);

            form.toggle_focused_field();

            form.handle_key_event(key(KeyCode::Char('b')));

            assert_eq!(form.get_description_value(), Some("b".to_string()));
        }
    }

    mod get_name_value {
        use super::*;

        #[test]
        fn returns_trimmed_name() {
            let form = ProjectForm::new(None, Some("  My Project  ".into()), None);
            let value = form.get_name_value();

            assert_eq!(value, "My Project");
        }

        #[test]
        fn returns_empty_string_when_missing() {
            let form = ProjectForm::new(None, None, None);

            assert_eq!(form.get_name_value(), "");
        }
    }

    mod get_description_value {
        use super::*;

        #[test]
        fn returns_some_trimmed_description() {
            let form = ProjectForm::new(None, Some("Name".into()), Some("  desc  ".into()));
            let value = form.get_description_value();

            assert_eq!(value, Some("desc".to_string()));
        }

        #[test]
        fn returns_none_when_empty() {
            let form = ProjectForm::new(None, Some("Name".into()), None);

            assert_eq!(form.get_description_value(), None);
        }

        #[test]
        fn returns_none_when_whitespace_only() {
            let form = ProjectForm::new(None, Some("Name".into()), Some("   ".into()));

            assert_eq!(form.get_description_value(), None);
        }
    }

    mod toggle_focused_field {
        use super::*;

        #[test]
        fn toggles_focus_flag() {
            let mut form = ProjectForm::new(None, None, None);

            assert!(form.is_name_focused);

            form.toggle_focused_field();
            assert!(!form.is_name_focused);

            form.toggle_focused_field();
            assert!(form.is_name_focused);
        }
    }

    mod validate_form {
        use super::*;

        #[test]
        fn invalid_when_name_empty() {
            let mut form = ProjectForm::new(None, None, None);

            let result = form.validate_form();

            assert!(!result);
        }

        #[test]
        fn valid_when_name_present() {
            let mut form = ProjectForm::new(None, Some("Project".into()), None);

            let result = form.validate_form();

            assert!(result);
        }
    }

    mod render {
        use super::*;
        use crate::tui::render_test_util::RenderTestUtil;

        #[test]
        fn form_with_values() {
            let form = ProjectForm::new(
                None,
                Some("My Project".to_string()),
                Some("My Description".to_string()),
            );

            let mut buf = Buffer::empty(Rect::new(0, 0, 61, 10));

            form.render(buf.area, &mut buf);

            let expected = vec![
                "   ┌──────────────────────────────────────────────── Esc ┐   ",
                "   │┌Name───────────────────────────────────────────────┐│   ",
                "   ││My Project                                         ││   ",
                "   │└───────────────────────────────────────────────────┘│   ",
                "   │┌Description────────────────────────────────────────┐│   ",
                "   ││My Description                                     ││   ",
                "   ││                                                   ││   ",
                "   ││                                                   ││   ",
                "   │└───────────────────────────────────────────────────┘│   ",
                "   └─────────────────────────────────────────────────────┘   ",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn empty_form() {
            let form = ProjectForm::new(None, None, None);
            let mut buf = Buffer::empty(Rect::new(0, 0, 61, 10));

            form.render(buf.area, &mut buf);

            let expected = vec![
                "   ┌──────────────────────────────────────────────── Esc ┐   ",
                "   │┌Name───────────────────────────────────────────────┐│   ",
                "   ││                                                   ││   ",
                "   │└───────────────────────────────────────────────────┘│   ",
                "   │┌Description────────────────────────────────────────┐│   ",
                "   ││                                                   ││   ",
                "   ││                                                   ││   ",
                "   ││                                                   ││   ",
                "   │└───────────────────────────────────────────────────┘│   ",
                "   └─────────────────────────────────────────────────────┘   ",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }
    }
}
