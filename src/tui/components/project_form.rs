use crate::core::app_error::AppError;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Color, Modifier, Stylize, Widget};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear};
use ratatui_textarea::TextArea;

pub struct ProjectForm<'a> {
    name_text_area: TextArea<'a>,
    description_text_area: TextArea<'a>,
    is_name_focused: bool,
}

impl<'a> ProjectForm<'a> {
    pub fn new(name: Option<String>, description: Option<String>) -> Self {
        let mut name_text_area = TextArea::new(vec![name.clone().unwrap_or_default()]);
        name_text_area.set_style(Style::default().fg(Color::DarkGray));

        name_text_area.set_block(
            Block::default()
                .border_style(Color::DarkGray)
                .borders(Borders::ALL)
                .title("Name"),
        );

        let mut description_text_area =
            TextArea::new(vec![description.clone().unwrap_or_default()]);

        description_text_area.set_style(Style::default().fg(Color::DarkGray));

        description_text_area.set_block(
            Block::default()
                .border_style(Color::DarkGray)
                .borders(Borders::ALL)
                .title("Description"),
        );

        description_text_area.set_cursor_style(Style::default());

        Self {
            name_text_area,
            description_text_area,
            is_name_focused: true,
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<ProjectFormEvent, AppError> {
        match key_event.code {
            KeyCode::Esc => return Ok(ProjectFormEvent::Cancel),
            KeyCode::Tab => self.toggle_focused_field(),
            KeyCode::Enter => {
                let is_valid = self.validate_form()?;
                if is_valid {
                    return Ok(ProjectFormEvent::Save {
                        name: self.get_name_value(),
                        description: self.get_description_value(),
                    });
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

        Ok(ProjectFormEvent::Consumed)
    }

    fn validate_form(&mut self) -> Result<bool, AppError> {
        let mut is_valid = true;

        let Some(block) = self.name_text_area.block().cloned() else {
            return Err(AppError::InvalidState {
                message: "No block on name text area",
            });
        };

        if self.get_name_value().len() == 0 {
            is_valid = false;
            self.name_text_area
                .set_block(block.border_style(Color::Red));
        } else {
            self.name_text_area
                .set_block(block.border_style(Color::DarkGray));
        }

        Ok(is_valid)
    }

    fn get_name_value(&self) -> String {
        self.name_text_area
            .lines()
            .first()
            .map(String::as_str)
            .unwrap_or("")
            .trim()
            .to_string()
    }

    fn get_description_value(&self) -> Option<String> {
        let description = self
            .description_text_area
            .lines()
            .first()
            .map(String::as_str)
            .unwrap_or("")
            .trim()
            .to_string();

        if description.len() == 0 {
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

impl<'a> Widget for &ProjectForm<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = " Esc ".blue().bold().into_right_aligned_line();

        let block = Block::bordered()
            .title(title)
            .bg(Color::LightYellow)
            .fg(Color::DarkGray);

        // define popup area first
        let popup_area = area.centered(Constraint::Length(60), Constraint::Length(10));
        let inner = block.inner(popup_area);

        // clear + render block on same area
        Clear.render(popup_area, buf);
        block.render(popup_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Fill(1)])
            .split(inner);

        self.name_text_area.render(chunks[0], buf);
        self.description_text_area.render(chunks[1], buf);
    }
}

pub enum ProjectFormEvent {
    Save {
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
            let mut form = ProjectForm::new(None, None);

            let event = form.handle_key_event(key(KeyCode::Esc)).unwrap();

            match event {
                ProjectFormEvent::Cancel => {}
                _ => panic!("expected Cancel"),
            }
        }

        #[test]
        fn tab_toggles_focus_and_consumes() {
            let mut form = ProjectForm::new(None, None);

            let event = form.handle_key_event(key(KeyCode::Tab)).unwrap();

            match event {
                ProjectFormEvent::Consumed => {}
                _ => panic!("expected Consumed"),
            }

            assert!(!form.is_name_focused);
        }

        #[test]
        fn enter_with_valid_name_returns_save() {
            let mut form = ProjectForm::new(Some("Project".into()), Some("desc".into()));

            let event = form.handle_key_event(key(KeyCode::Enter)).unwrap();

            match event {
                ProjectFormEvent::Save { name, description } => {
                    assert_eq!(name, "Project");
                    assert_eq!(description, Some("desc".to_string()));
                }
                _ => panic!("expected Save"),
            }
        }

        #[test]
        fn enter_with_invalid_name_returns_consumed_and_marks_invalid() {
            let mut form = ProjectForm::new(None, None);

            let event = form.handle_key_event(key(KeyCode::Enter)).unwrap();

            match event {
                ProjectFormEvent::Consumed => {}
                _ => panic!("expected Consumed"),
            }
        }

        #[test]
        fn typing_goes_to_name_field_when_focused() {
            let mut form = ProjectForm::new(None, None);

            form.handle_key_event(key(KeyCode::Char('a'))).unwrap();

            assert_eq!(form.get_name_value(), "a");
        }

        #[test]
        fn typing_goes_to_description_when_not_focused() {
            let mut form = ProjectForm::new(None, None);

            form.toggle_focused_field();

            form.handle_key_event(key(KeyCode::Char('b'))).unwrap();

            assert_eq!(form.get_description_value(), Some("b".to_string()));
        }
    }

    mod get_name_value {
        use super::*;

        #[test]
        fn returns_trimmed_name() {
            let form = ProjectForm::new(Some("  My Project  ".into()), None);
            let value = form.get_name_value();

            assert_eq!(value, "My Project");
        }

        #[test]
        fn returns_empty_string_when_missing() {
            let form = ProjectForm::new(None, None);

            assert_eq!(form.get_name_value(), "");
        }
    }

    mod get_description_value {
        use super::*;

        #[test]
        fn returns_some_trimmed_description() {
            let form = ProjectForm::new(Some("Name".into()), Some("  desc  ".into()));
            let value = form.get_description_value();

            assert_eq!(value, Some("desc".to_string()));
        }

        #[test]
        fn returns_none_when_empty() {
            let form = ProjectForm::new(Some("Name".into()), None);

            assert_eq!(form.get_description_value(), None);
        }

        #[test]
        fn returns_none_when_whitespace_only() {
            let form = ProjectForm::new(Some("Name".into()), Some("   ".into()));

            assert_eq!(form.get_description_value(), None);
        }
    }

    mod toggle_focused_field {
        use super::*;

        #[test]
        fn toggles_focus_flag() {
            let mut form = ProjectForm::new(None, None);

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
            let mut form = ProjectForm::new(None, None);

            let result = form.validate_form().unwrap();

            assert!(!result);
        }

        #[test]
        fn valid_when_name_present() {
            let mut form = ProjectForm::new(Some("Project".into()), None);

            let result = form.validate_form().unwrap();

            assert!(result);
        }
    }

    mod render {
        use super::*;

        fn flatten(buf: &Buffer) -> String {
            buf.content()
                .iter()
                .map(|c| c.symbol())
                .collect::<Vec<_>>()
                .join("")
        }

        #[test]
        fn render_snapshot() {
            let form = ProjectForm::new(
                Some("My Project".to_string()),
                Some("My Description".to_string()),
            );

            let mut buf = Buffer::empty(Rect::new(0, 0, 61, 10));

            (&form).render(buf.area, &mut buf);

            let flat = flatten(&buf);

            assert!(flat.contains("Esc"));
            assert!(flat.contains("Name"));
            assert!(flat.contains("Description"));
            assert!(flat.contains("My Project"));
            assert!(flat.contains("My Description"));
        }

        #[test]
        fn render_empty_form_snapshot() {
            let form = ProjectForm::new(None, None);

            let mut buf = Buffer::empty(Rect::new(0, 0, 61, 10));

            (&form).render(buf.area, &mut buf);

            let flat = flatten(&buf);

            assert!(flat.contains("Esc"));
            assert!(flat.contains("Name"));
            assert!(flat.contains("Description"));

            // empty values should still render structure
            assert!(flat.len() > 0);
        }
    }
}
