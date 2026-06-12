use crate::core::format::Format;
use crate::core::time_format::TimeFormat;
use crate::tui::terminal_user_interface::TuiError;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::{Color, Stylize, Widget};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear};
use ratatui_textarea::TextArea;

pub struct ManualSessionDialog<'a> {
    text_area: TextArea<'a>,
    time_format: TimeFormat,
}

impl<'a> ManualSessionDialog<'a> {
    pub fn new(time_format: TimeFormat) -> Self {
        let mut text_area = TextArea::new(vec!["".to_string()]);
        text_area.set_style(Style::default().fg(Color::DarkGray));
        text_area.set_block(Self::get_block(time_format));

        Self {
            text_area,
            time_format,
        }
    }

    fn get_block(time_format: TimeFormat) -> Block<'a> {
        Block::default()
            .border_style(Color::DarkGray)
            .borders(Borders::ALL)
            .title(Self::time_format_to_instruction(time_format))
    }

    fn time_format_to_instruction(time_format: TimeFormat) -> String {
        match time_format {
            TimeFormat::HoursMinutesSeconds => "Hours, minutes, and seconds [00:00:00]".to_string(),
            TimeFormat::HoursMinutes => "Hours and minutes [00:00]".to_string(),
            TimeFormat::DecimalHours => "Decimal hours [0.00]".to_string(),
            TimeFormat::Seconds => "Seconds".to_string(),
        }
    }

    pub fn handle_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> Result<ManualSessionEvent, TuiError> {
        match key_event.code {
            KeyCode::Esc => return Ok(ManualSessionEvent::Cancel),
            KeyCode::Enter => match self.get_value() {
                Ok(value) => {
                    return Ok(ManualSessionEvent::Save {
                        total_seconds: value,
                    });
                }
                Err(error) => {
                    self.mark_form_invalid(error)?;
                }
            },
            _ => {
                self.text_area.input(key_event);
            }
        }

        Ok(ManualSessionEvent::Consumed)
    }

    fn mark_form_invalid(&mut self, message: String) -> Result<(), TuiError> {
        let new_block = Self::get_block(self.time_format)
            .border_style(Color::Red)
            .title_bottom(message);

        self.text_area.set_block(new_block);

        Ok(())
    }

    fn get_value(&self) -> Result<i64, String> {
        let text = self
            .text_area
            .lines()
            .first()
            .map(String::as_str)
            .unwrap_or("")
            .trim();

        let time_format = self.time_format;

        Format::string_to_seconds(text, time_format)
    }
}

impl<'a> Widget for &ManualSessionDialog<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = " Esc ".blue().bold().into_right_aligned_line();

        let block = Block::bordered()
            .title(title)
            .bg(Color::LightYellow)
            .fg(Color::DarkGray);

        // define popup area first
        let popup_area = area.centered(Constraint::Length(60), Constraint::Length(5));
        let inner = block.inner(popup_area);

        // clear + render block on same area
        Clear.render(popup_area, buf);
        block.render(popup_area, buf);

        self.text_area.render(inner, buf);
    }
}

pub enum ManualSessionEvent {
    Save { total_seconds: i64 },
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

    // ----------------------------
    // KEY HANDLING
    // ----------------------------

    #[test]
    fn handle_escape_cancels() {
        let mut dialog = ManualSessionDialog::new(TimeFormat::Seconds);

        let res = dialog.handle_key_event(key(KeyCode::Esc)).unwrap();
        assert!(matches!(res, ManualSessionEvent::Cancel));
    }

    #[test]
    fn handle_enter_with_valid_value_saves_session() {
        let mut dialog = ManualSessionDialog::new(TimeFormat::Seconds);

        for c in "120".chars() {
            dialog.text_area.input(key(KeyCode::Char(c)));
        }

        let event = dialog.handle_key_event(key(KeyCode::Enter)).unwrap();

        assert!(matches!(
            event,
            ManualSessionEvent::Save { total_seconds: 120 }
        ));
    }

    #[test]
    fn handle_enter_with_invalid_value_renders_warning_message() {
        let mut dialog = ManualSessionDialog::new(TimeFormat::Seconds);

        for c in "abc".chars() {
            dialog.text_area.input(key(KeyCode::Char(c)));
        }

        let event = dialog.handle_key_event(key(KeyCode::Enter)).unwrap();

        assert!(matches!(event, ManualSessionEvent::Consumed));

        let mut buf = Buffer::empty(Rect::new(0, 0, 61, 5));
        dialog.render(buf.area, &mut buf);

        let rendered: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");

        assert!(
            rendered.contains("Expected whole seconds"),
            "Rendered buffer did not contain validation error:\n{}",
            rendered
        );
    }

    #[test]
    fn handle_enter_empty_is_error_and_consumed() {
        let mut dialog = ManualSessionDialog::new(TimeFormat::Seconds);

        let res = dialog.handle_key_event(key(KeyCode::Enter)).unwrap();
        assert!(matches!(res, ManualSessionEvent::Consumed));
    }

    #[test]
    fn handle_char_input_consumed() {
        let mut dialog = ManualSessionDialog::new(TimeFormat::Seconds);

        let res = dialog.handle_key_event(key(KeyCode::Char('1'))).unwrap();
        assert!(matches!(res, ManualSessionEvent::Consumed));
    }

    #[test]
    fn get_value() {
        let mut dialog = ManualSessionDialog::new(TimeFormat::Seconds);

        for c in "xyz".chars() {
            dialog.text_area.input(key(KeyCode::Char(c)));
        }

        let error = dialog.get_value().unwrap_err();

        assert_eq!(error, "Expected whole seconds (e.g. 120)");

        let mut dialog = ManualSessionDialog::new(TimeFormat::Seconds);

        for c in "123".chars() {
            dialog.text_area.input(key(KeyCode::Char(c)));
        }

        let result = dialog.get_value();
        assert!(result.is_ok());
    }

    // ----------------------------
    // RENDER TEST (STRICT SNAPSHOT)
    // ----------------------------

    #[test]
    fn render_snapshot() {
        let dialog = ManualSessionDialog::new(TimeFormat::HoursMinutesSeconds);

        let mut buf = Buffer::empty(Rect::new(0, 0, 61, 5));

        dialog.render(buf.area, &mut buf);

        let flat: String = buf
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("");

        assert!(flat.contains("Esc"));
        assert!(flat.contains("Hours, minutes, and seconds"));
    }

    // ----------------------------
    // INVALID FORM MARKING
    // ----------------------------

    #[test]
    fn mark_invalid_changes_footer() {
        let mut dialog = ManualSessionDialog::new(TimeFormat::Seconds);

        dialog
            .mark_form_invalid("Invalid input".to_string())
            .unwrap();

        let mut buf = Buffer::empty(Rect::new(0, 0, 61, 5));
        dialog.render(buf.area, &mut buf);

        let flat: String = buf
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("");

        // footer content must appear somewhere in rendered buffer
        assert!(flat.contains("Invalid input"));
    }
}
