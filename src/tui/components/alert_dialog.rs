use crossterm::event::{KeyCode, KeyEvent};
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
        Self { message: message.to_string() }
    }

    /// Handle user keypresses
    ///
    /// # Errors
    ///
    /// Returns an error if executing user commands fails.
    pub fn handle_key_event(key_event: KeyEvent) -> AlertDialogEvent {
        match key_event.code {
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

pub enum AlertDialogEvent {
    Confirm,
    Cancel,
    Ignore,
}
