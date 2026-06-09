use crate::core::tracking::TrackingError;
use crate::tui::components::project_table::ProjectTable;
use crate::tui::components::session_table::SessionTable;
use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::{
    buffer::Buffer, layout::Rect,
    widgets::Widget,
    DefaultTerminal


    ,
    Frame,
};
use rusqlite::Connection;
use std::time::{Duration, Instant};
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Eq, PartialEq)]
enum ActiveWidget {
    SessionTable,
    ProjectTable
}

pub struct TerminalUserInterface<'a> {
    exit: bool,
    session_table: SessionTable<'a>,
    project_table: ProjectTable<'a>,
    active_widget: ActiveWidget,
}

impl<'a> TerminalUserInterface<'a> {
    /// Returns a new TUI.
    ///
    /// # Errors
    ///
    /// If `SQLite` fails to query sessions.
    pub fn new(connection: &'a Connection) -> rusqlite::Result<Self> {
        let date = OffsetDateTime::now_utc().date();

        Ok(Self {
            session_table: SessionTable::new(date, connection)?,
            project_table: ProjectTable::new(connection)?,
            exit: false,
            active_widget: ActiveWidget::SessionTable,
        })
    }

    /// Launches the terminal user interface.
    ///
    /// This method repeatedly renders the UI and waits for keyboard input. It exits
    /// successfully after a key press is received.
    ///
    /// # Errors
    ///
    /// Returns an error if drawing to the terminal fails, or if reading an event
    /// from the terminal input fails.
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), TuiError> {
        let tick_rate = Duration::from_secs(1);
        let mut last_tick = Instant::now();

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());

            if event::poll(timeout)? {
                self.handle_events()?;
            }

            if last_tick.elapsed() >= tick_rate {
                self.tick();
                last_tick = Instant::now();
            }
        }

        Ok(())
    }

    fn tick(&mut self) {
        self.session_table.tick();
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> Result<(), TuiError> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<(), TuiError> {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.exit(),
            KeyCode::Char('1') => self.active_widget = ActiveWidget::ProjectTable,
            KeyCode::Char('2') => self.active_widget = ActiveWidget::SessionTable,

            _ => match self.active_widget {
                ActiveWidget::SessionTable => self.session_table.handle_key_event(key_event)?,
                ActiveWidget::ProjectTable => self.project_table.handle_key_event(key_event)?,
            },
        }

        Ok(())
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &mut TerminalUserInterface<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Fill(1), Constraint::Fill(1)])
            .split(area);

        self.project_table.render(chunks[0], buf, self.active_widget == ActiveWidget::ProjectTable);
        self.session_table.render(chunks[1], buf, self.active_widget == ActiveWidget::SessionTable);
    }
}

#[derive(Debug, Error)]
pub enum TuiError {
    #[error("Tracking error: {0}")]
    Tracking(#[from] TrackingError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Invalid state: {message}")]
    InvalidState { message: &'static str },
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use ratatui::style::Style;
//
//     #[test]
//     fn render() {
//         let app = App::default();
//         let mut buf = Buffer::empty(Rect::new(0, 0, 50, 4));
//
//         app.render(buf.area, &mut buf);
//
//         let mut expected = Buffer::with_lines(vec![
//             "┏━━━━━━━━━━━━━ Counter App Tutorial ━━━━━━━━━━━━━┓",
//             "┃                    Value: 0                    ┃",
//             "┃                                                ┃",
//             "┗━ Decrement <Left> Increment <Right> Quit <Q> ━━┛",
//         ]);
//         let title_style = Style::new().bold();
//         let counter_style = Style::new().yellow();
//         let key_style = Style::new().blue().bold();
//         expected.set_style(Rect::new(14, 0, 22, 1), title_style);
//         expected.set_style(Rect::new(28, 1, 1, 1), counter_style);
//         expected.set_style(Rect::new(13, 3, 6, 1), key_style);
//         expected.set_style(Rect::new(30, 3, 7, 1), key_style);
//         expected.set_style(Rect::new(43, 3, 4, 1), key_style);
//
//         assert_eq!(buf, expected);
//     }
//
//     #[test]
//     fn handle_key_event() {
//         let mut app = App::default();
//         app.handle_key_event(KeyCode::Right.into());
//         assert_eq!(app.counter, 1);
//
//         app.handle_key_event(KeyCode::Left.into());
//         assert_eq!(app.counter, 0);
//
//         let mut app = App::default();
//         app.handle_key_event(KeyCode::Char('q').into());
//         assert!(app.exit);
//     }
// }
