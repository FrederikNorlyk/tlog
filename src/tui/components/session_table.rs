use crate::core::app_error::AppError;
use crate::core::clipboard::clipboard_backend::ClipboardBackend;
use crate::core::config::Config;
use crate::core::time_format::TimeFormat;
use crate::core::tracking::{TimeAdjustmentOperation, Tracking};
use crate::model::session::Session;
use crate::tui::components::alert_dialog::{AlertDialog, AlertDialogEvent};
use crate::tui::components::keybinds_dialog::Keybind;
use crate::tui::components::manual_session_dialog::{ManualSessionDialog, ManualSessionEvent};
use crate::tui::components::project_select::{ProjectSelect, ProjectSelectEvent};
use crate::tui::terminal_user_interface::{KeyEventResult, KeybindOverlay};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint};
use ratatui::style::{Color, Style};
use ratatui::text::{Span, Text};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    widgets::{Block, Cell, Row, StatefulWidget, Table, TableState, Widget},
};
use rusqlite::Connection;
use time::macros::format_description;
use time::{Date, Duration, OffsetDateTime};

pub struct SessionTable<'a> {
    sessions: Vec<Session>,
    date: Date,
    table_state: TableState,
    connection: &'a Connection,
    time_format: TimeFormat,
    project_select: Option<ProjectSelect<'a>>,
    is_showing_reset_alert_dialog: bool,
    is_showing_copy_keybinds: bool,
    manual_session_dialog: Option<ManualSessionDialog<'a>>,
    clipboard: Box<dyn ClipboardBackend>,
    table_height: u16,
}

impl<'a> SessionTable<'a> {
    /// Return a new session table component
    ///
    /// # Errors
    ///
    /// If `SQLite` fails to query sessions
    pub fn new(
        connection: &'a Connection,
        time_format: TimeFormat,
        date: Date,
        is_showing_copy_keybinds: bool,
        clipboard: Box<dyn ClipboardBackend>,
    ) -> Result<Self, AppError> {
        let mut table_state = TableState::default();
        let tracking = Tracking::new(connection);
        let sessions = tracking.list_all_sessions(date, None)?;

        if !sessions.is_empty() {
            table_state.select_next();
        }

        Ok(Self {
            sessions,
            date,
            table_state,
            connection,
            time_format,
            project_select: None,
            is_showing_reset_alert_dialog: false,
            is_showing_copy_keybinds,
            manual_session_dialog: None,
            clipboard,
            table_height: 0,
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
            let duration = self.time_format.format(session.total_seconds);
            let name = session.project.name.as_str();
            let name_length = u16::try_from(name.len()).unwrap_or(u16::MAX);

            max_name_length = max_name_length.max(name_length);
            total_seconds += session.total_seconds;

            let mut row = Row::new(vec![
                Cell::from(name).bold(),
                Cell::from(description),
                Cell::from(Text::from(duration).alignment(Alignment::Right)),
            ]);

            if session.is_started {
                row = row.fg(Color::Yellow);
            }

            rows.push(row);
        }

        let footer = Row::new(vec![
            Cell::from("Total").bold(),
            Cell::from(""),
            Cell::from(self.time_format.format(total_seconds)).underlined(),
        ]);

        let duration_width = match self.time_format {
            TimeFormat::HoursMinutesSeconds | TimeFormat::Seconds => 8,
            TimeFormat::HoursMinutes | TimeFormat::DecimalHours => 5,
        };

        let widths = [
            Constraint::Length(max_name_length),
            Constraint::Fill(6),
            Constraint::Length(duration_width),
        ];

        let title = if self.date == OffsetDateTime::now_utc().date() {
            " [2] Today ".to_string()
        } else {
            let format = format_description!("[weekday repr:long], [day] [month repr:long] [year]");
            format!(
                " [2] {} ",
                self.date
                    .format(&format)
                    .unwrap_or_else(|_| self.date.to_string())
            )
        };

        let mut table_block = Block::bordered().title(title).border_set(border::THICK);

        if is_active {
            table_block = table_block.green();
        }

        let table = Table::new(rows, widths)
            .block(table_block)
            .row_highlight_style(Style::new().reversed())
            .footer(footer);

        StatefulWidget::render(table, area, buf, &mut self.table_state);

        self.table_height = area.height - 3; // Subtract block borders and "Total" footer

        if let Some(project_select) = &mut self.project_select {
            project_select.render(area, buf);
        }

        if self.is_showing_reset_alert_dialog {
            let dialog = AlertDialog::new("You are about to reset the session");
            dialog.render(area, buf);
        }

        if let Some(dialog) = &mut self.manual_session_dialog {
            dialog.render(area, buf);
        }
    }

    /// Handle user keypresses
    ///
    /// # Errors
    ///
    /// Returns an error if executing user commands fails.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<KeyEventResult, AppError> {
        if self.project_select.is_some() {
            return self.handle_project_select_key_event(key_event);
        } else if self.manual_session_dialog.is_some() {
            return self.handle_manual_session_dialog_key_event(key_event);
        } else if self.is_showing_reset_alert_dialog {
            return self.handle_alert_dialog_key_event(key_event);
        } else if self.is_showing_copy_keybinds {
            return self.handle_copy_key_event(key_event);
        }

        let has_selected_session = self.get_selected_session().is_some();
        let half_page = self.table_height.saturating_sub(1) / 2;
        let ctrl_key_is_held = key_event.modifiers.contains(KeyModifiers::CONTROL);

        let mut did_match = true;

        match key_event.code {
            KeyCode::Char('h') | KeyCode::Left => self.shift_date(Duration::days(-1))?,
            KeyCode::Char('l') | KeyCode::Right => self.shift_date(Duration::days(1))?,
            KeyCode::Char('j') | KeyCode::Down => self.table_state.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.table_state.select_previous(),
            KeyCode::Char('u') if ctrl_key_is_held => self.table_state.scroll_up_by(half_page),
            KeyCode::Char('d') if ctrl_key_is_held => self.table_state.scroll_down_by(half_page),
            KeyCode::PageUp => self.table_state.scroll_up_by(self.table_height),
            KeyCode::PageDown => self.table_state.scroll_down_by(self.table_height),
            KeyCode::Char('g') | KeyCode::Home => self.table_state.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.table_state.select_last(),
            KeyCode::Char('d') | KeyCode::Delete if has_selected_session => {
                self.is_showing_reset_alert_dialog = true;
            }
            KeyCode::Char('D') if has_selected_session => self.reset_session()?,
            KeyCode::Char(' ') if has_selected_session => self.toggle_session()?,
            KeyCode::Char('a') if ctrl_key_is_held && has_selected_session => {
                self.adjust_selected_session_by_fifteen_minutes(
                    TimeAdjustmentOperation::Increment,
                )?;
            }
            KeyCode::Char('x') if ctrl_key_is_held && has_selected_session => {
                self.adjust_selected_session_by_fifteen_minutes(
                    TimeAdjustmentOperation::Decrement,
                )?;
            }
            KeyCode::Char('a') => {
                self.project_select = Some(ProjectSelect::new(self.connection, self.date)?);
            }
            KeyCode::Char('e') if has_selected_session => {
                self.manual_session_dialog = Some(ManualSessionDialog::new(self.time_format));
            }
            KeyCode::Char('c') if has_selected_session => {
                self.is_showing_copy_keybinds = true;

                return Ok(KeyEventResult::ShowKeybindOverlay {
                    overlay: KeybindOverlay::CopySession,
                });
            }
            KeyCode::Char('f') => self.cycle_time_format()?,
            _ => did_match = false,
        }

        if did_match {
            return Ok(KeyEventResult::Consumed);
        }

        Ok(KeyEventResult::Unused)
    }

    fn handle_project_select_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> Result<KeyEventResult, AppError> {
        let Some(project_select) = &mut self.project_select else {
            return Err(AppError::InvalidState {
                message: "No project select",
            });
        };

        if key_event.code == KeyCode::Esc {
            self.project_select = None;
            return Ok(KeyEventResult::Consumed);
        }

        match project_select.handle_key_event(key_event)? {
            ProjectSelectEvent::Selected { project_id } => {
                let tracking = Tracking::new(self.connection);

                tracking.start(project_id)?;
                self.sessions = tracking.list_all_sessions(self.date, None)?;
                self.project_select = None;
            }
            ProjectSelectEvent::Ignore => {}
        }

        Ok(KeyEventResult::Consumed)
    }

    fn handle_manual_session_dialog_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> Result<KeyEventResult, AppError> {
        let Some(dialog) = &mut self.manual_session_dialog else {
            return Err(AppError::InvalidState {
                message: "No project select",
            });
        };
        match dialog.handle_key_event(key_event) {
            ManualSessionEvent::Save { total_seconds } => {
                self.set_manual_session(total_seconds)?;
                self.manual_session_dialog = None;
            }
            ManualSessionEvent::Cancel => {
                self.manual_session_dialog = None;
            }
            ManualSessionEvent::Consumed => {}
        }

        Ok(KeyEventResult::Consumed)
    }

    fn handle_alert_dialog_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> Result<KeyEventResult, AppError> {
        match AlertDialog::handle_key_code(key_event.code) {
            AlertDialogEvent::Confirm => {
                self.reset_session()?;
                self.is_showing_reset_alert_dialog = false;
                Ok(KeyEventResult::Consumed)
            }
            AlertDialogEvent::Cancel => {
                self.is_showing_reset_alert_dialog = false;
                Ok(KeyEventResult::Consumed)
            }
            AlertDialogEvent::Ignore => Ok(KeyEventResult::Unused),
        }
    }

    fn handle_copy_key_event(&mut self, key_event: KeyEvent) -> Result<KeyEventResult, AppError> {
        let mut did_match = true;

        match key_event.code {
            KeyCode::Char('c') => self.copy_to_clipboard(CopyContent::All)?,
            KeyCode::Char('n') => self.copy_to_clipboard(CopyContent::Name)?,
            KeyCode::Char('d') => self.copy_to_clipboard(CopyContent::Description)?,
            KeyCode::Char('t') => self.copy_to_clipboard(CopyContent::Time)?,
            KeyCode::Char('p') => self.copy_to_clipboard(CopyContent::Project)?,
            KeyCode::Esc => {}
            _ => did_match = false,
        }

        if did_match {
            self.is_showing_copy_keybinds = false;
            return Ok(KeyEventResult::Consumed);
        }

        Ok(KeyEventResult::Unused)
    }

    fn shift_date(&mut self, duration: Duration) -> Result<(), AppError> {
        // Overflowing current date is not an issue, so using expect here is fine.
        self.date = self
            .date
            .checked_add(duration)
            .expect("Could not shift date");

        let tracking = Tracking::new(self.connection);
        self.sessions = tracking.list_all_sessions(self.date, None)?;

        Ok(())
    }

    fn toggle_session(&mut self) -> Result<(), AppError> {
        let Some(session) = self.get_selected_session() else {
            return Err(AppError::InvalidState {
                message: "No selected session",
            });
        };

        let tracking = Tracking::new(self.connection);
        tracking.toggle(session.project.id)?;
        self.sessions = tracking.list_all_sessions(self.date, None)?;

        Ok(())
    }

    fn adjust_selected_session_by_fifteen_minutes(
        &mut self,
        operation: TimeAdjustmentOperation,
    ) -> Result<(), AppError> {
        let Some(session) = self.get_selected_session() else {
            return Err(AppError::InvalidState {
                message: "No selected session",
            });
        };

        let tracking = Tracking::new(self.connection);
        tracking.adjust_by_fifteen_minutes(session.project.id, self.date, operation)?;
        self.sessions = tracking.list_all_sessions(self.date, None)?;

        Ok(())
    }

    fn reset_session(&mut self) -> Result<(), AppError> {
        let Some(session) = self.get_selected_session() else {
            return Err(AppError::InvalidState {
                message: "No selected session",
            });
        };

        let tracking = Tracking::new(self.connection);
        tracking.reset(session.project.id, self.date)?;
        self.sessions = tracking.list_all_sessions(self.date, None)?;

        Ok(())
    }

    fn cycle_time_format(&mut self) -> Result<(), AppError> {
        self.time_format = self.time_format.get_next_format();
        Config::set_time_format(self.time_format)?;
        Ok(())
    }

    fn copy_to_clipboard(&mut self, copy_content: CopyContent) -> Result<(), AppError> {
        let Some(session) = self.get_selected_session() else {
            return Err(AppError::InvalidState {
                message: "No selected session",
            });
        };

        let text = match copy_content {
            CopyContent::All => {
                let mut output = Vec::new();
                output.push(Self::escape_semicolon(&session.project.name));
                if let Some(description) = &session.project.description {
                    output.push(Self::escape_semicolon(description));
                }

                output.push(self.time_format.format(session.total_seconds));

                output.join(";")
            }
            CopyContent::Name => Self::escape_semicolon(&session.project.name),
            CopyContent::Description => {
                if let Some(description) = &session.project.description {
                    Self::escape_semicolon(description)
                } else {
                    String::new()
                }
            }
            CopyContent::Time => self.time_format.format(session.total_seconds),
            CopyContent::Project => {
                let mut project = Self::escape_semicolon(&session.project.name);
                if let Some(description) = &session.project.description {
                    project.push_str(" - ");
                    project.push_str(Self::escape_semicolon(description).as_str());
                }

                project
            }
        };

        self.clipboard.set_text(text)?;

        Ok(())
    }

    fn escape_semicolon(s: &str) -> String {
        s.replace(';', "")
    }

    fn get_selected_session(&self) -> Option<&Session> {
        let selected_index = self.table_state.selected()?;

        self.sessions.get(selected_index)
    }

    fn set_manual_session(&mut self, total_seconds: i64) -> Result<(), AppError> {
        let Some(session) = self.get_selected_session() else {
            return Err(AppError::InvalidState {
                message: "No selected session",
            });
        };

        let tracking = Tracking::new(self.connection);

        tracking.set(session.project.id, self.date, total_seconds)?;
        self.sessions = tracking.list_all_sessions(self.date, None)?;

        Ok(())
    }

    #[must_use]
    pub fn get_keybinds() -> Vec<Keybind> {
        vec![
            Keybind::new("a".to_string(), "Track a new project".to_string()),
            Keybind::new("e".to_string(), "Edit tracked time".to_string()),
            Keybind::new("space".to_string(), "Toggle time tracking".to_string()),
            Keybind::new("d".to_string(), "Delete session".to_string()),
            Keybind::new(
                "ctrl+a".to_string(),
                "Increment session by 15 min".to_string(),
            ),
            Keybind::new(
                "ctrl+x".to_string(),
                "Decrement session by 15 min".to_string(),
            ),
            Keybind::new("delete".to_string(), "Delete session".to_string()),
            Keybind::new("D".to_string(), "Force delete session".to_string()),
            Keybind::new("c".to_string(), "Copy".to_string()),
            Keybind::new("f".to_string(), "Change time format".to_string()),
            Keybind::new("k".to_string(), "Select previous row".to_string()),
            Keybind::new("↑".to_string(), "Select previous row".to_string()),
            Keybind::new("j".to_string(), "Select next row".to_string()),
            Keybind::new("↓".to_string(), "Select next row".to_string()),
            Keybind::new("g".to_string(), "Select first row".to_string()),
            Keybind::new("home".to_string(), "Select first row".to_string()),
            Keybind::new("G".to_string(), "Select last row".to_string()),
            Keybind::new("end".to_string(), "Select last row".to_string()),
            Keybind::new("ctrl+u".to_string(), "Scroll up half a page".to_string()),
            Keybind::new("ctrl+d".to_string(), "Scroll down half a page".to_string()),
            Keybind::new("page up".to_string(), "Scroll up a page".to_string()),
            Keybind::new("page down".to_string(), "Scroll down a page".to_string()),
            Keybind::new("h".to_string(), "Select previous date".to_string()),
            Keybind::new("←".to_string(), "Select previous date".to_string()),
            Keybind::new("l".to_string(), "Select next date".to_string()),
            Keybind::new("→".to_string(), "Select next date".to_string()),
        ]
    }

    #[must_use]
    pub fn get_keybinds_hint_text() -> Vec<Span<'static>> {
        vec![
            " Use ".into(),
            "a".blue().bold(),
            " to track a new project, ".into(),
            "e".blue().bold(),
            " to edit time, ".into(),
            "d".blue().bold(),
            " to delete, ".into(),
            "space".blue().bold(),
            " to toggle tracking".into(),
        ]
    }
}

#[derive(Copy, Clone)]
enum CopyContent {
    All,
    Name,
    Description,
    Time,
    Project,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::clipboard::mock_clipboard::MockClipboard;
    use crate::db::db_test_context::DBTestContext;
    use crate::db::event_repository::EventRepository;
    use crate::db::manual_session_repository::ManualSessionRepository;
    use crate::db::project_repository::ProjectRepository;
    use crate::model::event::EventType;
    use crossterm::event::KeyModifiers;
    use time::{Month, PrimitiveDateTime, Time};

    fn initialize_context() -> DBTestContext {
        let context = DBTestContext::new().unwrap();
        let project_repository = ProjectRepository::new(context.connection());

        project_repository.insert("Project A", None).unwrap();

        project_repository
            .insert("Another project", Some("With a description"))
            .unwrap();

        project_repository
            .insert("One more", Some("With some other text"))
            .unwrap();

        let event_repository = EventRepository::new(context.connection());

        let start_date = get_test_date();
        let time = Time::from_hms(2, 30, 00).unwrap();

        let mut timestamp = PrimitiveDateTime::new(start_date, time)
            .assume_utc()
            .unix_timestamp();

        event_repository
            .insert(1, EventType::Start, timestamp)
            .unwrap();

        // 1 hour 30 min 30 seconds
        timestamp += 5430;

        event_repository
            .insert(1, EventType::Stop, timestamp)
            .unwrap();

        let manual_session_repository = ManualSessionRepository::new(context.connection());

        // 15 min = 900 sec
        manual_session_repository
            .upsert(2, start_date, 900)
            .unwrap();

        context
    }

    fn get_test_date() -> Date {
        Date::from_calendar_date(2024, Month::September, 20).unwrap()
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn get_keybinds() {
        let keybinds: Vec<String> = SessionTable::get_keybinds()
            .iter()
            .map(|key| format!("{key}"))
            .collect();

        let joined = keybinds.join(" ");

        assert_eq!(
            joined,
            "a - Track a new project e - Edit tracked time space - Toggle time tracking d - Delete session ctrl+a - Increment session by 15 min ctrl+x - Decrement session by 15 min delete - Delete session D - Force delete session c - Copy f - Change time format k - Select previous row ↑ - Select previous row j - Select next row ↓ - Select next row g - Select first row home - Select first row G - Select last row end - Select last row ctrl+u - Scroll up half a page ctrl+d - Scroll down half a page page up - Scroll up a page page down - Scroll down a page h - Select previous date ← - Select previous date l - Select next date → - Select next date"
        );
    }

    #[test]
    fn get_keybinds_hint_text() {
        let keybinds: Vec<String> = SessionTable::get_keybinds_hint_text()
            .iter()
            .map(|key| format!("{key}"))
            .collect();

        let joined = keybinds.join(" ");

        assert_eq!(
            joined,
            " Use  a  to track a new project,  e  to edit time,  d  to delete,  space  to toggle tracking"
        );
    }

    mod render {
        use super::*;
        use crate::tui::render_test_util::RenderTestUtil;

        #[test]
        fn table_of_sessions() {
            let context = initialize_context();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                get_test_date(),
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            let area = Rect::new(0, 0, 110, 6);
            let mut buf = Buffer::empty(area);

            table.render(area, &mut buf, true);

            let expected = vec![
                "┏ [2] Friday, 20 September 2024 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃Another project With a description                                                                     00:15┃",
                "┃Project A                                                                                              01:31┃",
                "┃                                                                                                            ┃",
                "┃Total                                                                                                  01:46┃",
                "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn today_in_title() {
            let context = initialize_context();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                OffsetDateTime::now_utc().date(),
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            let area = Rect::new(0, 0, 110, 6);
            let mut buf = Buffer::empty(area);

            table.render(area, &mut buf, true);

            let expected = vec![
                "┏ [2] Today ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃                                                                                                            ┃",
                "┃                                                                                                            ┃",
                "┃                                                                                                            ┃",
                "┃Total                                                                                                  00:00┃",
                "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn project_select() {
            let context = initialize_context();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                get_test_date(),
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            table.handle_key_event(key(KeyCode::Char('a'))).unwrap();

            let area = Rect::new(0, 0, 110, 10);
            let mut buf = Buffer::empty(area);

            table.render(area, &mut buf, true);

            let expected = vec![
                "┏ [2] Friday, 20 September 2024 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃Another project With a description                                                                     00:15┃",
                "┃Proje┌──────────────────────────────────────────────────────────────────────────────────────────── Esc ┐1:31┃",
                "┃     │                                                                                                 │    ┃",
                "┃     │                                                                                                 │    ┃",
                "┃     │                                                                                                 │    ┃",
                "┃     │One more - With some other text                                                                  │    ┃",
                "┃     │                                                                                                 │    ┃",
                "┃Total└─────────────────────────────────────────────────────────────────────────────────────────────────┘1:46┃",
                "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn reset_alert_dialog() {
            let context = initialize_context();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                get_test_date(),
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            table.handle_key_event(key(KeyCode::Char('d'))).unwrap();

            let area = Rect::new(0, 0, 110, 10);
            let mut buf = Buffer::empty(area);

            table.render(area, &mut buf, true);

            let expected = vec![
                "┏ [2] Friday, 20 September 2024 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃Another project With a description                                                                     00:15┃",
                "┃Project A               ┌───────────────────────────────────────────────────── Esc ┐                   01:31┃",
                "┃                        │You are about to reset the session                        │                        ┃",
                "┃                        │                                                          │                        ┃",
                "┃                        │                                                          │                        ┃",
                "┃                        │                                                          │                        ┃",
                "┃                        └────────────────y to delete, n to cancel ─────────────────┘                        ┃",
                "┃Total                                                                                                  01:46┃",
                "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn manual_session_dialog() {
            let context = initialize_context();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                get_test_date(),
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            table.handle_key_event(key(KeyCode::Char('e'))).unwrap();

            let area = Rect::new(0, 0, 110, 11);
            let mut buf = Buffer::empty(area);

            table.render(area, &mut buf, true);

            let expected = vec![
                "┏ [2] Friday, 20 September 2024 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃Another project With a description                                                                     00:15┃",
                "┃Project A                                                                                              01:31┃",
                "┃                        ┌───────────────────────────────────────────────────── Esc ┐                        ┃",
                "┃                        │┌Hours and minutes [00:00]───────────────────────────────┐│                        ┃",
                "┃                        ││                                                        ││                        ┃",
                "┃                        │└────────────────────────────────────────────────────────┘│                        ┃",
                "┃                        └──────────────────────────────────────────────────────────┘                        ┃",
                "┃                                                                                                            ┃",
                "┃Total                                                                                                  01:46┃",
                "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }
    }

    mod handle_key_event {
        use super::*;
        use std::thread;

        #[test]
        fn navigation() {
            let context = initialize_context();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                get_test_date(),
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            assert!(table.get_selected_session().is_some());

            // Go down by pressing 'j'
            let event = table.handle_key_event(key(KeyCode::Char('j'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert_eq!(table.get_selected_session().unwrap().project.id, 1);

            // Go up by pressing 'k'
            let event = table.handle_key_event(key(KeyCode::Char('k'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert_eq!(table.get_selected_session().unwrap().project.id, 2);

            // Go down by pressing Down arrow
            let event = table.handle_key_event(key(KeyCode::Down)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert_eq!(table.get_selected_session().unwrap().project.id, 1);

            // Go up by pressing Up arrow
            let event = table.handle_key_event(key(KeyCode::Up)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert_eq!(table.get_selected_session().unwrap().project.id, 2);

            // Go to previous date using 'h' key
            let event = table.handle_key_event(key(KeyCode::Char('h'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);

            assert_eq!(
                table.date,
                Date::from_calendar_date(2024, Month::September, 19).unwrap()
            );

            // Go to previous date using the left arrow
            let event = table.handle_key_event(key(KeyCode::Left)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);

            assert_eq!(
                table.date,
                Date::from_calendar_date(2024, Month::September, 18).unwrap()
            );

            // Go to next date using 'l' key
            let event = table.handle_key_event(key(KeyCode::Char('l'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);

            assert_eq!(
                table.date,
                Date::from_calendar_date(2024, Month::September, 19).unwrap()
            );

            // Go to next date using the right arrow
            let event = table.handle_key_event(key(KeyCode::Right)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);

            assert_eq!(
                table.date,
                Date::from_calendar_date(2024, Month::September, 20).unwrap()
            );
        }

        #[test]
        fn delete() {
            let context = initialize_context();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                get_test_date(),
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            let event = table.handle_key_event(key(KeyCode::Char('d'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.is_showing_reset_alert_dialog);

            let event = table.handle_key_event(key(KeyCode::Char('x'))).unwrap();
            assert_eq!(event, KeyEventResult::Unused);
            assert!(table.is_showing_reset_alert_dialog);

            let event = table.handle_key_event(key(KeyCode::Char('n'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(!table.is_showing_reset_alert_dialog);

            let event = table.handle_key_event(key(KeyCode::Char('d'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.is_showing_reset_alert_dialog);

            let event = table.handle_key_event(key(KeyCode::Esc)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(!table.is_showing_reset_alert_dialog);

            let event = table.handle_key_event(key(KeyCode::Char('d'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.is_showing_reset_alert_dialog);

            let event = table.handle_key_event(key(KeyCode::Char('y'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(!table.is_showing_reset_alert_dialog);
            assert_eq!(table.sessions.len(), 1);

            let event = table.handle_key_event(key(KeyCode::Char('D'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(!table.is_showing_reset_alert_dialog);
            assert!(table.sessions.is_empty());
        }

        #[test]
        fn toggle_session() {
            let context = initialize_context();
            let today = OffsetDateTime::now_utc().date();
            let manual_session_repository = ManualSessionRepository::new(context.connection());

            manual_session_repository.upsert(2, today, 500).unwrap();
            manual_session_repository.upsert(1, today, 500).unwrap();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                today,
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            // Assert that all sessions are stopped
            assert_eq!(table.sessions.len(), 2);

            let first = table.sessions.first().unwrap();
            let second = table.sessions.get(1).unwrap();

            assert_eq!(first.project.id, 2);
            assert!(!first.is_started);
            assert_eq!(second.project.id, 1);
            assert!(!second.is_started);

            // Start the selected session - by default the first row - which is project 2
            table.handle_key_event(key(KeyCode::Char(' '))).unwrap();

            // Assert that project 1 is started and project 2 is still stopped
            assert_eq!(table.sessions.len(), 2);

            let first = table.sessions.first().unwrap();
            let second = table.sessions.get(1).unwrap();

            assert_eq!(first.project.id, 2);
            assert_eq!(first.total_seconds, 500);
            assert!(first.is_started);
            assert_eq!(second.project.id, 1);
            assert_eq!(second.total_seconds, 500);
            assert!(!second.is_started);

            // Advance time by one second
            table.tick();
            thread::sleep(std::time::Duration::from_secs(1));

            // Verify that the started project (2) has increased its number of seconds by 1
            let first = table.sessions.first().unwrap();
            let second = table.sessions.get(1).unwrap();

            assert_eq!(first.total_seconds, 501);
            assert_eq!(second.total_seconds, 500);

            // Select the second row
            table.handle_key_event(key(KeyCode::Down)).unwrap();
            // Start the second session (project 1)
            table.handle_key_event(key(KeyCode::Char(' '))).unwrap();

            // Assert that project 1 has been started and that project 2 has been stopped
            assert_eq!(table.sessions.len(), 2);

            let first = table.sessions.first().unwrap();
            let second = table.sessions.get(1).unwrap();

            assert_eq!(first.project.id, 2);
            assert!(!first.is_started);
            assert_eq!(second.project.id, 1);
            assert!(second.is_started);

            // Advance time by two seconds
            table.tick();
            table.tick();
            thread::sleep(std::time::Duration::from_secs(2));

            // Stop the selected session (project 1)
            table.handle_key_event(key(KeyCode::Char(' '))).unwrap();

            // Assert that all projects have been stopped
            assert_eq!(table.sessions.len(), 2);

            let first = table.sessions.first().unwrap();
            let second = table.sessions.get(1).unwrap();

            assert_eq!(first.project.id, 2);
            assert!(!first.is_started);
            assert_eq!(second.project.id, 1);
            assert!(!second.is_started);
        }

        #[test]
        fn adjust_by_fifteen_minutes() {
            let context = initialize_context();
            let date = get_test_date();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                date,
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            // -------------------
            // Increment by 15 min
            // -------------------
            let ctrl_e = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
            let event = table.handle_key_event(ctrl_e).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);

            let tracking = Tracking::new(context.connection());
            let sessions = tracking.list_all_sessions(date, Some(2)).unwrap();
            let session = sessions.first().unwrap();

            assert_eq!(session.project.id, 2);
            assert!(!session.is_started);
            assert_eq!(session.total_seconds, 1800);

            // -------------------
            // Decrement by 15 min
            // -------------------
            let ctrl_x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
            let event = table.handle_key_event(ctrl_x).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);

            let tracking = Tracking::new(context.connection());
            let sessions = tracking.list_all_sessions(date, Some(2)).unwrap();
            let session = sessions.first().unwrap();

            assert_eq!(session.project.id, 2);
            assert!(!session.is_started);
            assert_eq!(session.total_seconds, 900);
        }

        #[test]
        fn manual_session() {
            let context = initialize_context();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                get_test_date(),
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            let event = table.handle_key_event(key(KeyCode::Char('e'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.manual_session_dialog.is_some());

            let event = table.handle_key_event(key(KeyCode::Char('Q'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.manual_session_dialog.is_some());

            let event = table.handle_key_event(key(KeyCode::Esc)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.manual_session_dialog.is_none());

            let event = table.handle_key_event(key(KeyCode::Char('e'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.manual_session_dialog.is_some());

            let event = table.handle_key_event(key(KeyCode::Char('1'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);

            let event = table.handle_key_event(key(KeyCode::Enter)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.manual_session_dialog.is_none());
        }

        #[test]
        fn project_select() {
            let context = initialize_context();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                get_test_date(),
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            assert!(table.project_select.is_none());

            let event = table.handle_key_event(key(KeyCode::Char('a'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.project_select.is_some());

            let event = table.handle_key_event(key(KeyCode::Esc)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.project_select.is_none());

            let event = table.handle_key_event(key(KeyCode::Char('a'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.project_select.is_some());

            let event = table.handle_key_event(key(KeyCode::Char('x'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.project_select.is_some());

            let event = table.handle_key_event(key(KeyCode::Enter)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.project_select.is_none());
        }

        #[test]
        fn is_showing_copy_keybinds() {
            let context = initialize_context();

            let mut table = SessionTable::new(
                context.connection(),
                TimeFormat::HoursMinutes,
                get_test_date(),
                false,
                Box::new(MockClipboard::default()),
            )
            .unwrap();

            // "Copy keybinds overlay" can be closed with Esc
            table.handle_key_event(key(KeyCode::Char('c'))).unwrap();
            let event = table.handle_key_event(key(KeyCode::Esc)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(!table.is_showing_copy_keybinds);

            // Overlay stays open with unused keys
            table.handle_key_event(key(KeyCode::Char('c'))).unwrap();
            let event = table.handle_key_event(key(KeyCode::Char('x'))).unwrap();
            assert_eq!(event, KeyEventResult::Unused);
            assert!(table.is_showing_copy_keybinds);

            // 'c' copies all cells
            assert_clipboard_event(&mut table, 'c', "Another project;With a description;00:15");

            // 'n' copies name
            table.handle_key_event(key(KeyCode::Char('c'))).unwrap();
            assert_clipboard_event(&mut table, 'n', "Another project");
            assert!(!table.is_showing_copy_keybinds);

            // 'd' copies description
            table.handle_key_event(key(KeyCode::Char('c'))).unwrap();
            assert_clipboard_event(&mut table, 'd', "With a description");
            assert!(!table.is_showing_copy_keybinds);

            // 't' copies time
            table.handle_key_event(key(KeyCode::Char('c'))).unwrap();
            assert_clipboard_event(&mut table, 't', "00:15");
            assert!(!table.is_showing_copy_keybinds);

            // 'p' copies project's name and description
            table.handle_key_event(key(KeyCode::Char('c'))).unwrap();
            assert_clipboard_event(&mut table, 'p', "Another project - With a description");
            assert!(!table.is_showing_copy_keybinds);

            // 'f' cycles time format
            table.handle_key_event(key(KeyCode::Char('f'))).unwrap();
            table.handle_key_event(key(KeyCode::Char('c'))).unwrap();
            assert_clipboard_event(&mut table, 't', "00.25");
            assert!(!table.is_showing_copy_keybinds);

            // 'f' cycles time format
            table.handle_key_event(key(KeyCode::Char('f'))).unwrap();
            table.handle_key_event(key(KeyCode::Char('c'))).unwrap();
            assert_clipboard_event(&mut table, 't', "900");
            assert!(!table.is_showing_copy_keybinds);

            // 'f' cycles time format
            table.handle_key_event(key(KeyCode::Char('f'))).unwrap();
            table.handle_key_event(key(KeyCode::Char('c'))).unwrap();
            assert_clipboard_event(&mut table, 't', "00:15:00");
            assert!(!table.is_showing_copy_keybinds);
        }

        fn assert_clipboard_event(
            table: &mut SessionTable,
            pressed_key: char,
            expected_text: &str,
        ) {
            let event = table
                .handle_key_event(key(KeyCode::Char(pressed_key)))
                .unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            let clipped_text = table.clipboard.get_text().unwrap();
            assert_eq!(clipped_text, expected_text);
        }
    }
}
