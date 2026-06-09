use crate::core::tracking::Tracking;
use crate::model::session::Session;
use crate::tui::components::alert_dialog::{AlertDialog, AlertDialogEvent};
use crate::tui::components::project_select::{ProjectSelect, ProjectSelectEvent};
use crate::tui::terminal_user_interface::TuiError;
use crate::tui::terminal_user_interface::TuiError::InvalidState;
use crate::util::format_util::FormatUtil;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Constraint;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Clear, Shadow};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Cell, Row, StatefulWidget, Table, TableState, Widget},
};
use rusqlite::Connection;
use time::Date;

pub struct SessionTable<'a> {
    sessions: Vec<Session>,
    date: Date,
    state: TableState,
    connection: &'a Connection,
    project_select: Option<ProjectSelect<'a>>,
    is_showing_reset_alert_dialog: bool,
}

impl<'a> SessionTable<'a> {
    /// Return a new session table component
    ///
    /// # Errors
    ///
    /// If `SQLite` fails to query sessions
    pub fn new(date: Date, connection: &'a Connection) -> rusqlite::Result<Self> {
        let mut state = TableState::default();
        let tracking = Tracking::new(connection);
        let sessions = tracking.list_all_sessions(date)?;

        if !sessions.is_empty() {
            state.select_next();
        }

        Ok(Self {
            sessions,
            date,
            state,
            connection,
            project_select: None,
            is_showing_reset_alert_dialog: false,
        })
    }

    pub fn tick(&mut self) {
        for session in &mut self.sessions {
            if session.is_started {
                session.total_seconds += 1;
            }
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, is_active: bool) {
        let mut total_seconds = 0;
        let mut max_name_length: u16 = 5;

        let mut rows: Vec<Row> = Vec::new();

        for session in &self.sessions {
            let description = session.project.description.as_deref().unwrap_or_default();
            let duration = FormatUtil::seconds_to_duration(session.total_seconds);
            let name = session.project.name.as_str();
            let name_length = u16::try_from(name.len()).unwrap_or(u16::MAX);

            max_name_length = max_name_length.max(name_length);
            total_seconds += session.total_seconds;

            let mut row = Row::new(vec![
                Cell::from(name).bold(),
                Cell::from(description),
                Cell::from(duration),
            ]);

            if session.is_started {
                row = row.fg(Color::Yellow);
            }

            rows.push(row);
        }

        let footer = Row::new(vec![
            Cell::from("Total").bold(),
            Cell::from(""),
            Cell::from(FormatUtil::seconds_to_duration(total_seconds)).underlined(),
        ]);

        let widths = [
            Constraint::Length(max_name_length),
            Constraint::Fill(6),
            Constraint::Length(8),
        ];

        let title = Line::from(" [2] Sessions ");

        let mut table_block = Block::bordered().title(title).border_set(border::THICK);

        if is_active {
            let instructions = Line::from(vec![
                " Use ".into(),
                "g/G".blue().bold(),
                " to go top/bottom, ".into(),
                "space".blue().bold(),
                " to toggle tracking, ".into(),
                "s".blue().bold(),
                " to track a new project, ".into(),
                "d".blue().bold(),
                " to delete ".into(),
            ]);

            table_block = table_block.title_bottom(instructions.centered()).green();
        }

        let table = Table::new(rows, widths)
            .block(table_block)
            .row_highlight_style(Style::new().reversed())
            .footer(footer);

        StatefulWidget::render(table, area, buf, &mut self.state);

        if let Some(project_select) = &mut self.project_select {
            let shadow = Shadow::overlay().black().on_yellow();
            let popup_title = " Esc ".blue().bold().into_right_aligned_line();

            let popup_block = Block::bordered()
                .title(popup_title)
                .shadow(shadow)
                .bg(Color::LightYellow)
                .fg(Color::DarkGray);

            let centered_area =
                area.centered(Constraint::Percentage(60), Constraint::Percentage(60));

            // clears out any background in the area before rendering the popup
            Widget::render(Clear, centered_area, buf);
            project_select.render(centered_area, buf, popup_block);
        }

        if self.is_showing_reset_alert_dialog {
            let dialog = AlertDialog::new("You are about to reset the session");
            dialog.render(area, buf);
        }
    }

    /// Handle user keypresses
    ///
    /// # Errors
    ///
    /// Returns an error if executing user commands fails.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<(), TuiError> {
        if let Some(project_select) = &mut self.project_select {
            if key_event.code == KeyCode::Esc {
                self.project_select = None;
                return Ok(());
            }

            match project_select.handle_key_event(key_event)? {
                ProjectSelectEvent::Selected { project_id } => {
                    let tracking = Tracking::new(self.connection);
                    tracking.start(project_id)?;
                    self.sessions = tracking.list_all_sessions(self.date)?;
                    self.project_select = None;
                }
                ProjectSelectEvent::Ignore => {}
            }

            return Ok(());
        } else if self.is_showing_reset_alert_dialog {
            match AlertDialog::handle_key_event(key_event) {
                AlertDialogEvent::Confirm => {
                    self.reset_session()?;
                    self.is_showing_reset_alert_dialog = false
                }
                AlertDialogEvent::Cancel => self.is_showing_reset_alert_dialog = false,
                AlertDialogEvent::Ignore => {}
            }
            return Ok(());
        }

        let has_selected_session = self.get_selected_session().is_some();

        match key_event.code {
            KeyCode::Char('j') | KeyCode::Down => self.state.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.state.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.state.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.state.select_last(),
            KeyCode::Char('d') if has_selected_session => self.is_showing_reset_alert_dialog = true,
            KeyCode::Char('D') if has_selected_session => self.reset_session()?,
            KeyCode::Char(' ') if has_selected_session => self.toggle_session()?,
            KeyCode::Char('s') => {
                self.project_select = Some(ProjectSelect::new(self.connection, self.date)?)
            }
            _ => {}
        }

        Ok(())
    }

    fn toggle_session(&mut self) -> Result<(), TuiError> {
        let Some(session) = self.get_selected_session() else {
            return Err(InvalidState {
                message: "No selected session",
            });
        };

        let tracking = Tracking::new(self.connection);
        tracking.toggle(session.project.id)?;
        self.sessions = tracking.list_all_sessions(self.date)?;

        Ok(())
    }

    fn reset_session(&mut self) -> Result<(), TuiError> {
        let Some(session) = self.get_selected_session() else {
            return Err(InvalidState {
                message: "No selected session",
            });
        };

        let tracking = Tracking::new(self.connection);
        tracking.reset(session.project.id, self.date)?;
        self.sessions = tracking.list_all_sessions(self.date)?;

        Ok(())
    }

    fn get_selected_session(&self) -> Option<&Session> {
        let Some(selected_index) = self.state.selected() else {
            return None;
        };

        let session = &self.sessions[selected_index];

        Some(session)
    }
}
