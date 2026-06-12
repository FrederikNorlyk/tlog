use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Line;
use ratatui::style::{Color, Stylize};
use ratatui::widgets::{Block, Clear, Paragraph, Shadow, Widget, Wrap};

pub struct AlertDialog {
    message: String,
}

impl AlertDialog {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }

    /// Handle user keypresses
    ///
    /// # Errors
    ///
    /// Returns an error if executing user commands fails.
    pub fn handle_key_code(code: KeyCode) -> AlertDialogEvent {
        match code {
            KeyCode::Char('y') => AlertDialogEvent::Confirm,
            KeyCode::Esc | KeyCode::Char('n') => AlertDialogEvent::Cancel,
            _ => AlertDialogEvent::Ignore,
        }
    }
}

impl Widget for AlertDialog {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let shadow = Shadow::overlay().black().on_yellow();
        let title = " Esc ".blue().bold().into_right_aligned_line();

        let instructions = Line::from(vec![
            "y".blue().bold(),
            " to delete, ".into(),
            "n".blue().bold(),
            " to cancel ".into(),
        ])
        .centered();

        let block = Block::bordered()
            .title(title)
            .title_bottom(instructions)
            .shadow(shadow)
            .bg(Color::LightYellow)
            .fg(Color::DarkGray);

        let centered_area = area.centered(Constraint::Length(60), Constraint::Length(6));

        // clears out any background in the area before rendering the popup
        Widget::render(Clear, centered_area, buf);

        let paragraph = Paragraph::new(self.message)
            .wrap(Wrap { trim: true })
            .block(block);

        paragraph.render(centered_area, buf);
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum AlertDialogEvent {
    Confirm,
    Cancel,
    Ignore,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_contains_required_elements() {
        let dialog = AlertDialog::new("You are about to reset the session");

        // must be large enough for centered layout + shadow
        let mut buf = Buffer::empty(Rect::new(0, 0, 61, 7));

        dialog.render(buf.area, &mut buf);

        let rendered = buf
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("");

        // --- core content ---
        assert!(rendered.contains("You are about to reset the session"));

        // --- title (top-right aligned in your widget) ---
        assert!(rendered.contains("Esc"));

        // --- footer instructions ---
        assert!(rendered.contains("y"));
        assert!(rendered.contains("n"));
        assert!(rendered.contains("to delete"));
        assert!(rendered.contains("cancel"));
    }

    #[test]
    fn handle_key_event() {
        assert_keycode(KeyCode::Esc, AlertDialogEvent::Cancel);
        assert_keycode(KeyCode::Char('n'), AlertDialogEvent::Cancel);
        assert_keycode(KeyCode::Char('y'), AlertDialogEvent::Confirm);
        assert_keycode(KeyCode::Char('x'), AlertDialogEvent::Ignore);
        assert_keycode(KeyCode::Enter, AlertDialogEvent::Ignore);
    }

    fn assert_keycode(code: KeyCode, expected_event: AlertDialogEvent) {
        let event = AlertDialog::handle_key_code(code);
        assert_eq!(event, expected_event);
    }
}
