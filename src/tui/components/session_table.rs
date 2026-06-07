use crate::core::tracking::Tracking;
use crate::model::session::Session;
use crate::tui::terminal_user_interface::TuiError;
use crate::tui::terminal_user_interface::TuiError::InvalidState;
use crate::util::format_util::FormatUtil;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Constraint;
use ratatui::style::{Color, Style};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Cell, Row, StatefulWidget, Table, TableState, Widget},
};
use time::Date;

pub struct SessionTable<'a> {
    sessions: Vec<Session>,
    date: Date,
    state: TableState,
    tracking: &'a Tracking<'a>,
}

impl<'a> SessionTable<'a> {
    /// Return a new session table component
    ///
    /// # Errors
    ///
    /// If `SQLite` fails to query sessions
    pub fn new(date: Date, tracking: &'a Tracking) -> rusqlite::Result<Self> {
        let mut state = TableState::default();
        let sessions = tracking.list_all_sessions(date)?;

        if !sessions.is_empty() {
            state.select_next();
        }

        Ok(Self {
            sessions,
            date,
            state,
            tracking,
        })
    }

    pub fn tick(&mut self) {
        for session in &mut self.sessions {
            if session.is_started {
                session.total_seconds += 1;
            }
        }
    }

    /// Handle user keypresses
    ///
    /// # Errors
    ///
    /// Returns an error if executing user commands fails.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<(), TuiError> {
        match key_event.code {
            KeyCode::Char('j') | KeyCode::Down => self.state.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.state.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.state.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.state.select_last(),
            KeyCode::Char(' ') => {
                self.toggle_session()?;
            }

            _ => {}
        }

        Ok(())
    }

    fn toggle_session(&mut self) -> Result<(), TuiError> {
        let Some(selected_index) = self.state.selected() else {
            return Err(InvalidState {
                message: "No selected session",
            });
        };

        let session = &self.sessions[selected_index];
        self.tracking.toggle(session.project.id)?;
        self.sessions = self.tracking.list_all_sessions(self.date)?;

        Ok(())
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, block: Block) {
        let inner = block.inner(area);
        block.render(area, buf);

        let rows = self.sessions.iter().map(|session| {
            let (hours, minutes, seconds) = FormatUtil::seconds_to_hms(session.total_seconds);
            let description = session.project.description.as_deref().unwrap_or_default();
            let duration = format!("{hours:02}:{minutes:02}:{seconds:02}");

            let row = Row::new(vec![
                Cell::from(session.project.name.as_str()),
                Cell::from(description),
                Cell::from(duration),
            ]);

            if session.is_started {
                row.fg(Color::Yellow)
            } else {
                row
            }
        });

        let widths = [
            Constraint::Length(5),
            Constraint::Length(25),
            Constraint::Length(8),
        ];

        let title = Line::from(" Sessions ".bold());

        let instructions = Line::from(vec![
            " Use ".into(),
            "g/G".blue().bold(),
            " to go top/bottom, ".into(),
            "space".blue().bold(),
            " to select, ".into(),
        ]);

        let table_block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let table = Table::new(rows, widths)
            .block(table_block)
            .row_highlight_style(Style::new().reversed());

        StatefulWidget::render(table, inner, buf, &mut self.state);
    }
}
