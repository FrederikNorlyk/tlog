use crate::core::app_error::AppError;
use crate::core::clipboard::system_clipboard::SystemClipboard;
use crate::core::config::Config;
use crate::tui::components::keybinds_dialog::{Keybind, KeybindDialogEvent, KeybindsDialog};
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

#[derive(Eq, PartialEq)]
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
                KeybindDialogEvent::Consumed => return Ok(()),
            }
        } else if key_event.code == KeyCode::Char('?') {
            let keybinds = match self.active_widget {
                ActiveWidget::SessionTable => vec![Keybind::new(
                    "a".to_string(),
                    "Track a new project".to_string(),
                )],
                ActiveWidget::ProjectTable => vec![Keybind::new(
                    "a".to_string(),
                    "Add a new project".to_string(),
                )],
            };
            self.keybind_dialog = Some(KeybindsDialog::new(keybinds));
            return Ok(());
        }

        let result = match self.active_widget {
            ActiveWidget::SessionTable => self.session_table.handle_key_event(key_event)?,
            ActiveWidget::ProjectTable => self.project_table.handle_key_event(key_event)?,
        };

        // Hide any potential overlay
        if result == KeyEventResult::Consumed {
            self.active_overlay = None;
        }

        match result {
            KeyEventResult::Unused => match key_event.code {
                KeyCode::Char('q' | 'Q') => self.exit(),
                KeyCode::Char('1') => self.active_widget = ActiveWidget::ProjectTable,
                KeyCode::Char('2') => self.active_widget = ActiveWidget::SessionTable,
                _ => {}
            },
            KeyEventResult::Consumed => {}
            KeyEventResult::ShowKeybindOverlay { overlay } => {
                self.active_overlay = Some(overlay);
            }
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
            ActiveWidget::ProjectTable => vec![
                " Use ".into(),
                "a".blue().bold(),
                " to add, ".into(),
                "e".blue().bold(),
                " to edit, ".into(),
                "d".blue().bold(),
                " to delete, ".into(),
            ],
            ActiveWidget::SessionTable => vec![
                " Use ".into(),
                "a".blue().bold(),
                " to track a new project, ".into(),
                "e".blue().bold(),
                " to edit time, ".into(),
                "d".blue().bold(),
                " to delete, ".into(),
                "space".blue().bold(),
                " to toggle tracking, ".into(),
            ],
        };

        spans.extend(["?".blue().bold(), " for keybinds ".into()]);

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
