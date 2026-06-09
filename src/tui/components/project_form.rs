use crate::tui::terminal_user_interface::TuiError;
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
        let mut name_input = TextArea::new(vec![name.clone().unwrap_or_default()]);
        name_input.set_style(Style::default().fg(Color::DarkGray));

        name_input.set_block(
            Block::default()
                .border_style(Color::DarkGray)
                .borders(Borders::ALL)
                .title("Name"),
        );

        let mut description_input = TextArea::new(vec![description.clone().unwrap_or_default()]);
        description_input.set_style(Style::default().fg(Color::DarkGray));

        description_input.set_block(
            Block::default()
                .border_style(Color::DarkGray)
                .borders(Borders::ALL)
                .title("Description"),
        );

        description_input.set_cursor_style(Style::default());

        Self {
            name_text_area: name_input,
            description_text_area: description_input,
            is_name_focused: true,
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<ProjectFormEvent, TuiError> {
        match key_event.code {
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

        Ok(ProjectFormEvent::Ignore)
    }

    fn validate_form(&mut self) -> Result<bool, TuiError> {
        let mut is_valid = true;

        let Some(block) = self.name_text_area.block().cloned() else {
            return Err(TuiError::InvalidState {
                message: "No block on name text area",
            });
        };

        if self.get_name_value().len() == 0 {
            is_valid = false;
            self.name_text_area.set_block(block.border_style(Color::Red));
        } else {
            self.name_text_area
                .set_block(block.border_style(Color::DarkGray));
        }

        let Some(block) = self.description_text_area.block().cloned() else {
            return Err(TuiError::InvalidState {
                message: "No block on description text area",
            });
        };

        if self.get_description_value().len() == 0 {
            is_valid = false;
            self.description_text_area
                .set_block(block.border_style(Color::Red));
        } else {
            self.description_text_area
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

    fn get_description_value(&self) -> String {
        self.description_text_area
            .lines()
            .first()
            .map(String::as_str)
            .unwrap_or("")
            .to_string()
    }

    fn toggle_focused_field(&mut self) {
        self.is_name_focused = !self.is_name_focused;

        if self.is_name_focused {
            self.name_text_area.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            self.description_text_area.set_cursor_style(Style::default());
        } else {
            self.description_text_area.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
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
    Save { name: String, description: String },
    Ignore,
}
