use crate::core::app_error::AppError;
use crate::core::clipboard::system_clipboard::SystemClipboard;
use crate::core::config::Config;
use crate::tui::components::keybinds_dialog::{KeybindDialogEvent, KeybindsDialog};
use crate::tui::components::project_table::ProjectTable;
use crate::tui::components::session_table::SessionTable;
use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::{DefaultTerminal, Frame, buffer::Buffer, layout::Rect, widgets::Widget};
use rusqlite::Connection;
use std::time::{Duration, Instant};
use time::OffsetDateTime;

#[derive(Debug, Eq, PartialEq)]
enum ActiveWidget {
    SessionTable,
    ProjectTable,
}

pub struct TerminalUserInterface<'a> {
    exit: bool,
    session_table: SessionTable<'a>,
    project_table: ProjectTable<'a>,
    active_widget: ActiveWidget,
    active_overlay: Option<KeybindOverlay>,
    keybind_dialog: Option<KeybindsDialog<'a>>,
}

impl<'a> TerminalUserInterface<'a> {
    /// Returns a new TUI.
    ///
    /// # Errors
    ///
    /// If `SQLite` fails to query sessions.
    pub fn new(connection: &'a Connection) -> Result<Self, AppError> {
        let time_format = Config::get()?.time_format();
        let date = OffsetDateTime::now_utc().date();

        Ok(Self {
            session_table: SessionTable::new(
                connection,
                time_format,
                date,
                false,
                Box::new(SystemClipboard::new()?),
            )?,
            project_table: ProjectTable::new(connection)?,
            exit: false,
            active_widget: ActiveWidget::SessionTable,
            active_overlay: None,
            keybind_dialog: None,
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
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), AppError> {
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

    fn handle_events(&mut self) -> Result<(), AppError> {
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

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<(), AppError> {
        if let Some(keybind_dialog) = &mut self.keybind_dialog {
            match keybind_dialog.handle_key_event(key_event) {
                KeybindDialogEvent::Closed => self.keybind_dialog = None,
                KeybindDialogEvent::Consumed => {}
            }
            return Ok(());
        } else if key_event.code == KeyCode::Char('?') {
            self.show_keybind_dialog_for_active_widget();
            return Ok(());
        }

        let result = match self.active_widget {
            ActiveWidget::SessionTable => self.session_table.handle_key_event(key_event)?,
            ActiveWidget::ProjectTable => self.project_table.handle_key_event(key_event)?,
        };

        match result {
            KeyEventResult::Unused => match key_event.code {
                KeyCode::Char('q' | 'Q') => self.exit = true,
                KeyCode::Char('1') => self.active_widget = ActiveWidget::ProjectTable,
                KeyCode::Char('2') => self.active_widget = ActiveWidget::SessionTable,
                _ => {}
            },
            KeyEventResult::Consumed => {
                self.active_overlay = None; // Hide any potential overlay
            }
            KeyEventResult::ShowKeybindOverlay { overlay } => {
                self.active_overlay = Some(overlay);
            }
        }

        Ok(())
    }

    fn show_keybind_dialog_for_active_widget(&mut self) {
        let keybinds = match self.active_widget {
            ActiveWidget::SessionTable => SessionTable::get_keybinds(),
            ActiveWidget::ProjectTable => ProjectTable::get_keybinds(),
        };

        self.keybind_dialog = Some(KeybindsDialog::new(keybinds));
    }
}

impl Widget for &mut TerminalUserInterface<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1)])
            .split(area);

        let main_area = chunks[0];

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Fill(1), Constraint::Fill(1)])
            .split(main_area);

        self.project_table.render(
            main_chunks[0],
            buf,
            self.active_widget == ActiveWidget::ProjectTable,
        );

        self.session_table.render(
            main_chunks[1],
            buf,
            self.active_widget == ActiveWidget::SessionTable,
        );

        let keybind_hint_area = chunks[1];

        let mut spans = match self.active_widget {
            ActiveWidget::ProjectTable => ProjectTable::get_keybinds_hint_text(),
            ActiveWidget::SessionTable => SessionTable::get_keybinds_hint_text(),
        };

        spans.extend([
            ", and ".into(),
            "?".blue().bold(),
            " to show keybinds ".into(),
        ]);
        Paragraph::new(Line::from(spans)).render(keybind_hint_area, buf);

        let version_text = format!("v {}", env!("CARGO_PKG_VERSION"));
        let version_line = Line::from(version_text).yellow().right_aligned();
        Paragraph::new(version_line).render(keybind_hint_area, buf);

        if let Some(overlay) = &self.active_overlay {
            let overlay_area = Rect {
                x: area.x,
                y: area.bottom().saturating_sub(3),
                width: area.width,
                height: 3,
            };

            let paragraph_area = Rect {
                x: area.x,
                y: area.bottom().saturating_sub(2),
                width: area.width,
                height: 1,
            };

            let instructions = match overlay {
                KeybindOverlay::CopySession => Line::from(vec![
                    "c".blue().bold(),
                    " copy all info, ".into(),
                    "n".blue().bold(),
                    " copy name, ".into(),
                    "d".blue().bold(),
                    " copy description ".into(),
                    "t".blue().bold(),
                    " copy time ".into(),
                    "p".blue().bold(),
                    " copy project ".into(),
                ]),
            };

            Clear.render(overlay_area, buf);

            let block = Block::new().bg(Color::DarkGray);
            block.render(overlay_area, buf);

            Paragraph::new(instructions)
                .alignment(ratatui::layout::Alignment::Center)
                .render(paragraph_area, buf);
        }

        if let Some(keybind_dialog) = &mut self.keybind_dialog {
            keybind_dialog.render(area, buf);
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum KeyEventResult {
    Unused,
    Consumed,
    ShowKeybindOverlay { overlay: KeybindOverlay },
}

#[derive(Debug, Eq, PartialEq)]
pub enum KeybindOverlay {
    CopySession,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::db_test_context::DBTestContext;
    use crate::db::event_repository::EventRepository;
    use crate::db::manual_session_repository::ManualSessionRepository;
    use crate::db::project_repository::ProjectRepository;
    use crate::model::event::EventType;
    use crossterm::event::KeyModifiers;

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

        let now = OffsetDateTime::now_utc();
        let today = now.date();
        let mut timestamp = now.unix_timestamp();

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
        manual_session_repository.upsert(2, today, 900).unwrap();

        context
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    mod handle_key_event {
        use super::*;

        #[test]
        fn keybind_dialog() {
            let context = DBTestContext::new().unwrap();
            let mut tui = TerminalUserInterface::new(context.connection()).unwrap();

            assert!(tui.keybind_dialog.is_none());
            tui.handle_key_event(key(KeyCode::Char('?'))).unwrap();
            assert!(tui.keybind_dialog.is_some());

            // Typing into the dialog keeps it open
            tui.handle_key_event(key(KeyCode::Char('j'))).unwrap();
            assert!(tui.keybind_dialog.is_some());
            // Pressing Esc closes the dialog
            tui.handle_key_event(key(KeyCode::Esc)).unwrap();
            assert!(tui.keybind_dialog.is_none());
        }

        #[test]
        fn active_widget() {
            let context = DBTestContext::new().unwrap();
            let mut tui = TerminalUserInterface::new(context.connection()).unwrap();

            assert_eq!(tui.active_widget, ActiveWidget::SessionTable);
            tui.handle_key_event(key(KeyCode::Char('1'))).unwrap();
            assert_eq!(tui.active_widget, ActiveWidget::ProjectTable);
            tui.handle_key_event(key(KeyCode::Char('2'))).unwrap();
            assert_eq!(tui.active_widget, ActiveWidget::SessionTable);
        }

        #[test]
        fn quit() {
            let context = DBTestContext::new().unwrap();
            let mut tui = TerminalUserInterface::new(context.connection()).unwrap();

            assert!(!tui.exit);
            tui.handle_key_event(key(KeyCode::Char('q'))).unwrap();
            assert!(tui.exit);

            tui.exit = false;
            tui.handle_key_event(key(KeyCode::Char('Q'))).unwrap();
            assert!(tui.exit);
        }
    }

    mod render {
        use super::*;
        use crate::db::db_test_context::DBTestContext;
        use crate::tui::render_test_util::RenderTestUtil;

        fn assert_render(tui: &mut TerminalUserInterface, hint_text: &str, expected_ui: Vec<&str>) {
            let screen_width = 120;
            let area = Rect::new(0, 0, screen_width, 10);
            let mut buf = Buffer::empty(area);

            tui.render(area, &mut buf);

            // The number of spaced between hint text and version tag is dynamic, so we calculate it using the screen width
            let version_text = format!("v {}", env!("CARGO_PKG_VERSION"));
            let padding = screen_width.saturating_sub(u16::try_from(hint_text.len()).unwrap());

            let expected_last_line = format!(
                "{hint_text}{version_text:>width$}",
                width = padding as usize
            );

            let mut expected = expected_ui;
            expected.push(expected_last_line.as_str());

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn keyboard_hint_text_for_session_table() {
            let context = DBTestContext::new().unwrap();
            let mut tui = TerminalUserInterface::new(context.connection()).unwrap();
            let hint_text = " Use a to track a new project, e to edit time, d to delete, space to toggle tracking, and ? to show keybinds";

            let expected = vec![
                "┏ [1] Projects ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓┏ [2] Today ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃Total                                             00:00:00┃",
                "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
            ];

            assert_render(&mut tui, hint_text, expected);
        }

        #[test]
        fn keyboard_hint_text_for_project_table() {
            let context = DBTestContext::new().unwrap();
            let mut tui = TerminalUserInterface::new(context.connection()).unwrap();
            let hint_text = " Use a to add, e to edit, d to delete, and ? to show keybinds";

            // Select the project table
            tui.handle_key_event(key(KeyCode::Char('1'))).unwrap();

            let expected = vec![
                "┏ [1] Projects ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓┏ [2] Today ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃Total                                             00:00:00┃",
                "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
            ];

            assert_render(&mut tui, hint_text, expected);
        }

        #[test]
        fn with_data() {
            let context = initialize_context();
            let mut tui = TerminalUserInterface::new(context.connection()).unwrap();
            let hint_text = " Use a to track a new project, e to edit time, d to delete, space to toggle tracking, and ? to show keybinds";

            let expected = vec![
                "┏ [1] Projects ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓┏ [2] Today ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃Another project With a description                        ┃┃Another project With a description                00:15:00┃",
                "┃One more        With some other text                      ┃┃Project A                                         01:30:30┃",
                "┃Project A                                                 ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃Total                                             01:45:30┃",
                "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
            ];

            assert_render(&mut tui, hint_text, expected);
        }

        #[test]
        fn copy_keybind_overlay() {
            let context = initialize_context();
            let mut tui = TerminalUserInterface::new(context.connection()).unwrap();

            tui.handle_key_event(key(KeyCode::Char('c'))).unwrap();

            let area = Rect::new(0, 0, 120, 10);
            let mut buf = Buffer::empty(area);

            tui.render(area, &mut buf);

            let expected = vec![
                "┏ [1] Projects ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓┏ [2] Today ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃Another project With a description                        ┃┃Another project With a description                00:15:00┃",
                "┃One more        With some other text                      ┃┃Project A                                         01:30:30┃",
                "┃Project A                                                 ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "┃                                                          ┃┃                                                          ┃",
                "                                                                                                                        ",
                "                      c copy all info, n copy name, d copy description t copy time p copy project                       ",
                "                                                                                                                        ",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }
    }
}
