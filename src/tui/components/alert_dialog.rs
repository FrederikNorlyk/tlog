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
    #[must_use]
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
    #[must_use]
    pub fn handle_key_code(code: KeyCode) -> AlertDialogEvent {
        match code {
            KeyCode::Char('y') => AlertDialogEvent::Confirm,
            KeyCode::Esc | KeyCode::Char('n') => AlertDialogEvent::Cancel,
            _ => AlertDialogEvent::Ignore,
        }
    }
}

impl Widget for AlertDialog {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = " Esc ".blue().bold().into_right_aligned_line();

        let instructions = Line::from(vec![
            "y".blue().bold(),
            " to delete, ".into(),
            "n".blue().bold(),
            " to cancel ".into(),
        ])
        .centered();

        let shadow = Shadow::overlay().black().on_yellow();

        let block = Block::bordered()
            .title(title)
            .title_bottom(instructions)
            .shadow(shadow)
            .bg(Color::LightYellow)
            .fg(Color::DarkGray);

        let centered_area = area.centered(Constraint::Length(60), Constraint::Length(6));

        // clears out any background in the area before rendering the popup
        Clear.render(centered_area, buf);

        let paragraph = Paragraph::new(self.message)
            .wrap(Wrap { trim: true })
            .block(block);

        paragraph.render(centered_area, buf);
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum AlertDialogEvent {
    Confirm,
    Cancel,
    Ignore,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::render_test_util::RenderTestUtil;

    #[test]
    fn render() {
        let dialog = AlertDialog::new("You are about to reset the session");
        let mut buf = Buffer::empty(Rect::new(0, 0, 61, 7));

        dialog.render(buf.area, &mut buf);

        let expected = vec![
            "                                                             ",
            " ┌───────────────────────────────────────────────────── Esc ┐",
            " │You are about to reset the session                        │",
            " │                                                          │",
            " │                                                          │",
            " │                                                          │",
            " └────────────────y to delete, n to cancel ─────────────────┘",
        ];

        RenderTestUtil::assert_eq(expected, &buf);
    }

    #[test]
    fn handle_key_code() {
        assert_key_code(KeyCode::Esc, AlertDialogEvent::Cancel);
        assert_key_code(KeyCode::Char('n'), AlertDialogEvent::Cancel);
        assert_key_code(KeyCode::Char('y'), AlertDialogEvent::Confirm);
        assert_key_code(KeyCode::Char('x'), AlertDialogEvent::Ignore);
        assert_key_code(KeyCode::Enter, AlertDialogEvent::Ignore);
    }

    fn assert_key_code(code: KeyCode, expected_event: AlertDialogEvent) {
        let event = AlertDialog::handle_key_code(code);
        assert_eq!(event, expected_event);
    }
}
