use crate::db::project_repository::ProjectRepository;
use crate::model::project::Project;
use crate::tui::terminal_user_interface::TuiError;
use crate::tui::terminal_user_interface::TuiError::InvalidState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Block, List, ListDirection, ListItem, ListState, StatefulWidget, Widget};
use ratatui_textarea::TextArea;
use rusqlite::Connection;
use time::Date;

pub struct ProjectSelect<'a> {
    connection: &'a Connection,
    state: ListState,
    projects: Vec<Project>,
    text_area: TextArea<'a>,
    date: Date,
}

impl<'a> ProjectSelect<'a> {
    pub fn new(connection: &'a Connection, date: Date) -> rusqlite::Result<Self> {
        let project_repository = ProjectRepository::new(connection);

        let mut state = ListState::default();
        state.select_first();

        Ok(Self {
            connection,
            state,
            projects: project_repository.search_by_name("", date)?,
            text_area: TextArea::new(vec!["".to_string()]),
            date,
        })
    }

    /// Handle user keypresses
    ///
    /// # Errors
    ///
    /// Returns an error if executing user commands fails.
    pub fn handle_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> Result<ProjectSelectEvent, TuiError> {
        let ctrl_key_is_held = key_event.modifiers.contains(KeyModifiers::CONTROL);

        match key_event.code {
            KeyCode::Down => {
                self.state.select_previous();
            }
            KeyCode::Up => {
                self.state.select_next();
            }
            KeyCode::Char('j') if ctrl_key_is_held => {
                self.state.select_previous();
            }
            KeyCode::Char('k') if ctrl_key_is_held => {
                self.state.select_next();
            }
            KeyCode::Enter => {
                return Ok(ProjectSelectEvent::Selected {
                    project_id: self.get_selected_project_id()?,
                });
            }
            _ => {
                self.text_area.input(key_event);
                self.state.select_first();

                let query = {
                    self.text_area
                        .lines()
                        .first()
                        .map(String::as_str)
                        .unwrap_or("")
                        .to_string()
                };

                self.search_projects(query.as_str())?;
            }
        }

        Ok(ProjectSelectEvent::Ignore)
    }

    fn get_selected_project_id(&mut self) -> Result<i32, TuiError> {
        let Some(selected_index) = self.state.selected() else {
            return Err(InvalidState {
                message: "No selected project",
            });
        };

        let project = &self.projects[selected_index];

        Ok(project.id)
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, block: Block) {
        let inner = block.inner(area);
        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1)])
            .split(inner);

        let items = self.projects.iter().map(|project| {
            let mut text = project.name.clone();

            if let Some(description) = &project.description {
                text.push_str(" - ");
                text.push_str(description);
            }

            ListItem::new(Text::from(text))
        });

        let list = List::new(items)
            .highlight_style(Style::new().reversed())
            .direction(ListDirection::BottomToTop);

        StatefulWidget::render(list, chunks[0], buf, &mut self.state);

        self.text_area.render(chunks[1], buf);
    }

    fn search_projects(&mut self, query: &str) -> rusqlite::Result<()> {
        let project_repository = ProjectRepository::new(self.connection);
        self.projects = project_repository.search_by_name(query, self.date)?;

        Ok(())
    }
}

pub enum ProjectSelectEvent {
    Selected { project_id: i32 },
    Ignore,
}
