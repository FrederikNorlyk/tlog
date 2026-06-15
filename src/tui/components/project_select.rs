use crate::core::app_error::AppError;
use crate::db::project_repository::ProjectRepository;
use crate::model::project::Project;
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
    ) -> Result<ProjectSelectEvent, AppError> {
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
                if let Some(project_id) = self.get_selected_project_id() {
                    return Ok(ProjectSelectEvent::Selected { project_id });
                }
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

    fn get_selected_project_id(&mut self) -> Option<i32> {
        let Some(selected_index) = self.state.selected() else {
            return None;
        };

        if self.projects.len() <= selected_index {
            return None;
        }

        let project = &self.projects[selected_index];

        Some(project.id)
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

#[derive(Debug, Eq, PartialEq)]
pub enum ProjectSelectEvent {
    Selected { project_id: i32 },
    Ignore,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_utils::DBTestContext;
    use time::macros::date;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(code: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(code), KeyModifiers::CONTROL)
    }

    fn flatten(buf: &Buffer) -> String {
        buf.content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("")
    }

    fn initialze_context() -> DBTestContext {
        let context = DBTestContext::new().unwrap();
        let project_repository = ProjectRepository::new(context.connection());

        project_repository.insert("Project A", None).unwrap();

        project_repository
            .insert("Another project", Some("With a description"))
            .unwrap();

        project_repository
            .insert("One more", Some("With some other text"))
            .unwrap();

        context
    }

    mod new {
        use super::*;

        #[test]
        fn creates_with_empty_query() {
            let context = initialze_context();

            let select = ProjectSelect::new(&context.connection(), date!(2024 - 01 - 01));

            assert!(select.is_ok());
        }
    }

    mod handle_key_event {
        use super::*;

        #[test]
        fn navigation_up_and_down() {
            let context = initialze_context();

            let mut select =
                ProjectSelect::new(&context.connection(), date!(2024 - 01 - 01)).unwrap();

            let previous_index = select.state.selected().unwrap() as i32;

            select.handle_key_event(ctrl_key('k')).unwrap();
            assert_eq!(select.state.selected().unwrap() as i32, previous_index + 1);

            select.handle_key_event(ctrl_key('j')).unwrap();
            assert_eq!(select.state.selected().unwrap() as i32, previous_index);

            select.handle_key_event(key(KeyCode::Up)).unwrap();
            assert_eq!(select.state.selected().unwrap() as i32, previous_index + 1);

            select.handle_key_event(key(KeyCode::Down)).unwrap();
            assert_eq!(select.state.selected().unwrap() as i32, previous_index);
        }

        #[test]
        fn first_project_selected_by_default() {
            let context = initialze_context();

            let mut select =
                ProjectSelect::new(&context.connection(), date!(2024 - 01 - 01)).unwrap();

            let result = select.handle_key_event(key(KeyCode::Enter)).unwrap();

            assert!(matches!(
                result,
                ProjectSelectEvent::Selected { project_id } if project_id == 2
            ));
        }

        #[test]
        fn typing_updates_query_and_refreshes_list() {
            let context = initialze_context();

            let mut select =
                ProjectSelect::new(&context.connection(), date!(2024 - 01 - 01)).unwrap();

            select.handle_key_event(key(KeyCode::Char('x'))).unwrap();

            assert_eq!(select.projects.len(), 1);
            assert_eq!(select.projects.first().unwrap().name, "One more");
        }
    }

    mod get_selected_project_id {
        use super::*;

        #[test]
        fn returns_selected_id() {
            let context = initialze_context();

            let mut select =
                ProjectSelect::new(&context.connection(), date!(2024 - 01 - 01)).unwrap();

            select.handle_key_event(key(KeyCode::Up)).unwrap();

            let project_id = select.get_selected_project_id().unwrap();

            assert_eq!(project_id, 3);
        }

        #[test]
        fn returns_none_when_no_selection() {
            let context = initialze_context();

            let mut select =
                ProjectSelect::new(&context.connection(), date!(2024 - 01 - 01)).unwrap();

            select.state.select(None);

            let result = select.get_selected_project_id();

            assert!(result.is_none());
        }
    }

    mod render {
        use super::*;

        #[test]
        fn renders_list_and_input() {
            let context = initialze_context();

            let mut select =
                ProjectSelect::new(&context.connection(), date!(2024 - 01 - 01)).unwrap();

            let block = Block::default();

            let area = Rect::new(0, 0, 60, 10);
            let mut buf = Buffer::empty(area);

            select.render(area, &mut buf, block);

            let flat = flatten(&buf);

            assert!(flat.contains("Project A"));
            assert!(flat.contains("One more - With some other text"));
            assert!(flat.contains("Another project - With a description"));
        }
    }
}
