use crate::core::app_error::AppError;
use crate::db::project_repository::ProjectRepository;
use crate::model::project::Project;
use crate::tui::components::alert_dialog::{AlertDialog, AlertDialogEvent};
use crate::tui::components::project_form::{ProjectForm, ProjectFormEvent};
use crate::tui::terminal_user_interface::KeyEventResult;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::symbols::border;
use ratatui::text::Line;
use ratatui::widgets::{Block, Cell, Row, StatefulWidget, Table, TableState, Widget};
use rusqlite::Connection;

pub struct ProjectTable<'a> {
    projects: Vec<Project>,
    state: TableState,
    connection: &'a Connection,
    is_showing_deletion_alert_dialog: bool,
    project_form: Option<ProjectForm<'a>>,
}

impl<'a> ProjectTable<'a> {
    /// Returns a new Project table.
    ///
    /// # Errors
    /// Returns an error if querying project fails.
    pub fn new(connection: &'a Connection) -> Result<Self, AppError> {
        let project_repository = ProjectRepository::new(connection);

        let mut projects = Vec::new();

        project_repository.for_each(|project| {
            projects.push(project);
            Ok(())
        })?;

        Ok(Self {
            projects,
            state: TableState::default(),
            connection,
            is_showing_deletion_alert_dialog: false,
            project_form: None,
        })
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, is_active: bool) {
        let mut max_name_length: u16 = 5;

        let mut rows: Vec<Row> = Vec::new();

        for project in &self.projects {
            let description = project.description.as_deref().unwrap_or_default();
            let name = project.name.as_str();
            let name_length = u16::try_from(name.len()).unwrap_or(u16::MAX);

            max_name_length = max_name_length.max(name_length);

            rows.push(Row::new(vec![
                Cell::from(name).bold(),
                Cell::from(description),
            ]));
        }

        let widths = [Constraint::Length(max_name_length), Constraint::Fill(1)];

        let title = Line::from(" [1] Projects ");

        let mut table_block = Block::bordered().title(title).border_set(border::THICK);

        if is_active {
            let instructions = Line::from(vec![
                " Use ".into(),
                "g/G".blue().bold(),
                " to go top/bottom, ".into(),
                "a".blue().bold(),
                " to add a new project ".into(),
            ])
            .centered();

            table_block = table_block.title_bottom(instructions).green();
        }

        let table = Table::new(rows, widths)
            .block(table_block)
            .row_highlight_style(Style::new().reversed());

        StatefulWidget::render(table, area, buf, &mut self.state);

        if let Some(form) = &mut self.project_form {
            form.render(area, buf);
        }

        if self.is_showing_deletion_alert_dialog {
            let dialog = AlertDialog::new("You are about to delete a project");
            dialog.render(area, buf);
        }
    }

    /// Handle user keypresses
    ///
    /// # Errors
    ///
    /// Returns an error if executing user commands fails.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<KeyEventResult, AppError> {
        if self.is_showing_deletion_alert_dialog {
            match AlertDialog::handle_key_code(key_event.code) {
                AlertDialogEvent::Confirm => {
                    self.delete_project()?;
                    self.is_showing_deletion_alert_dialog = false;
                }
                AlertDialogEvent::Cancel => {
                    self.is_showing_deletion_alert_dialog = false;
                }
                AlertDialogEvent::Ignore => {}
            }

            return Ok(KeyEventResult::Consumed);
        } else if let Some(form) = &mut self.project_form {
            match form.handle_key_event(key_event) {
                ProjectFormEvent::Save { name, description } => {
                    self.insert_project(name.as_str(), description.as_deref())?;
                    self.project_form = None;
                }
                ProjectFormEvent::Cancel => {
                    self.project_form = None;
                }
                ProjectFormEvent::Consumed => {}
            }

            return Ok(KeyEventResult::Consumed);
        }

        let has_selected_project = self.get_selected_project().is_some();

        let mut did_match = true;

        match key_event.code {
            KeyCode::Char('j') | KeyCode::Down => self.state.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.state.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.state.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.state.select_last(),
            KeyCode::Char('a') => self.project_form = Some(ProjectForm::new(None, None)),
            KeyCode::Char('d') if has_selected_project => {
                self.is_showing_deletion_alert_dialog = true;
            }
            KeyCode::Char('D') if has_selected_project => self.delete_project()?,
            _ => did_match = false,
        }

        if did_match {
            return Ok(KeyEventResult::Consumed);
        }

        Ok(KeyEventResult::Unused)
    }

    fn delete_project(&mut self) -> Result<(), AppError> {
        let Some(project) = self.get_selected_project() else {
            return Err(AppError::InvalidState {
                message: "No selected project",
            });
        };

        let project_repository = ProjectRepository::new(self.connection);
        project_repository.delete(project.id)?;
        self.refresh_projects()?;

        Ok(())
    }

    fn insert_project(&mut self, name: &str, description: Option<&str>) -> Result<(), AppError> {
        let project_repository = ProjectRepository::new(self.connection);
        project_repository.insert(name, description)?;
        self.refresh_projects()?;

        Ok(())
    }

    fn refresh_projects(&mut self) -> Result<(), AppError> {
        let project_repository = ProjectRepository::new(self.connection);

        self.projects.clear();

        project_repository.for_each(|project| {
            self.projects.push(project);
            Ok(())
        })?;

        Ok(())
    }

    fn get_selected_project(&self) -> Option<&Project> {
        let selected_index = self.state.selected()?;

        self.projects.get(selected_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::db_test_context::DBTestContext;
    use crossterm::event::KeyModifiers;

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

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    mod render {
        use super::*;
        use crate::tui::render_test_util::RenderTestUtil;

        #[test]
        fn table_of_projects() {
            let context = initialze_context();
            let mut table = ProjectTable::new(context.connection()).unwrap();
            let area = Rect::new(0, 0, 60, 5);
            let mut buf = Buffer::empty(area);

            table.render(area, &mut buf, true);

            let expected = vec![
                "┏ [1] Projects ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃Another project With a description                        ┃",
                "┃One more        With some other text                      ┃",
                "┃Project A                                                 ┃",
                "┗━━━━ Use g/G to go top/bottom, a to add a new project ━━━━┛",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn project_form() {
            let context = initialze_context();
            let mut table = ProjectTable::new(context.connection()).unwrap();
            table.handle_key_event(key(KeyCode::Char('a'))).unwrap();
            let area = Rect::new(0, 0, 70, 17);
            let mut buf = Buffer::empty(area);

            table.render(area, &mut buf, true);

            let expected = vec![
                "┏ [1] Projects ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃Another project With a description                                  ┃",
                "┃One more        With some other text                                ┃",
                "┃Project A                                                           ┃",
                "┃    ┌───────────────────────────────────────────────────── Esc ┐    ┃",
                "┃    │┌Name────────────────────────────────────────────────────┐│    ┃",
                "┃    ││                                                        ││    ┃",
                "┃    │└────────────────────────────────────────────────────────┘│    ┃",
                "┃    │┌Description─────────────────────────────────────────────┐│    ┃",
                "┃    ││                                                        ││    ┃",
                "┃    ││                                                        ││    ┃",
                "┃    ││                                                        ││    ┃",
                "┃    │└────────────────────────────────────────────────────────┘│    ┃",
                "┃    └──────────────────────────────────────────────────────────┘    ┃",
                "┃                                                                    ┃",
                "┃                                                                    ┃",
                "┗━━━━━━━━━ Use g/G to go top/bottom, a to add a new project ━━━━━━━━━┛",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn delete_dialog() {
            let context = initialze_context();
            let mut table = ProjectTable::new(context.connection()).unwrap();
            table.handle_key_event(key(KeyCode::Down)).unwrap();
            table.handle_key_event(key(KeyCode::Char('d'))).unwrap();
            let area = Rect::new(0, 0, 70, 13);
            let mut buf = Buffer::empty(area);

            table.render(area, &mut buf, true);

            let expected = vec![
                "┏ [1] Projects ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓",
                "┃Another project With a description                                  ┃",
                "┃One more        With some other text                                ┃",
                "┃Project A                                                           ┃",
                "┃    ┌───────────────────────────────────────────────────── Esc ┐    ┃",
                "┃    │You are about to delete a project                         │    ┃",
                "┃    │                                                          │    ┃",
                "┃    │                                                          │    ┃",
                "┃    │                                                          │    ┃",
                "┃    └────────────────y to delete, n to cancel ─────────────────┘    ┃",
                "┃                                                                    ┃",
                "┃                                                                    ┃",
                "┗━━━━━━━━━ Use g/G to go top/bottom, a to add a new project ━━━━━━━━━┛",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }
    }

    mod handle_key_event {
        use super::*;

        #[test]
        fn is_showing_delete_alert_dialog() {
            let context = initialze_context();
            let mut table = ProjectTable::new(context.connection()).unwrap();

            // Select first row and press 'd' to show delete dialog
            assert!(!table.is_showing_deletion_alert_dialog);
            table.handle_key_event(key(KeyCode::Down)).unwrap();
            table.handle_key_event(key(KeyCode::Char('d'))).unwrap();
            assert!(table.is_showing_deletion_alert_dialog);

            // Verify keys unused by the dialog doesn't close it
            let event = table.handle_key_event(key(KeyCode::Char('x'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.is_showing_deletion_alert_dialog);

            // Verify that the dialog can be closed, and that it does not trigger deletion
            let event = table.handle_key_event(key(KeyCode::Char('n'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(!table.is_showing_deletion_alert_dialog);
            assert_eq!(table.projects.len(), 3);

            // Press 'd' to show delete dialog
            table.handle_key_event(key(KeyCode::Char('d'))).unwrap();
            assert!(table.is_showing_deletion_alert_dialog);

            // Verify that confirming deletion deletes a project
            table.handle_key_event(key(KeyCode::Char('y'))).unwrap();
            assert!(!table.is_showing_deletion_alert_dialog);
            assert_eq!(table.projects.len(), 2);
        }

        #[test]
        fn is_showing_project_form() {
            let context = initialze_context();
            let mut table = ProjectTable::new(context.connection()).unwrap();

            // Press 'a' to show project form
            assert!(table.project_form.is_none());
            table.handle_key_event(key(KeyCode::Char('a'))).unwrap();
            assert!(table.project_form.is_some());

            // Verify that typing text is consumed by the form
            let event = table.handle_key_event(key(KeyCode::Char('u'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.project_form.is_some());

            // Verify that pressing Esc closes the form
            let event = table.handle_key_event(key(KeyCode::Esc)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            assert!(table.project_form.is_none());

            // Press 'a' to show project form
            table.handle_key_event(key(KeyCode::Char('a'))).unwrap();
            assert!(table.project_form.is_some());

            // Fill out description
            table.handle_key_event(key(KeyCode::Tab)).unwrap();
            table.handle_key_event(key(KeyCode::Char('a'))).unwrap();

            // Verify that name is required
            table.handle_key_event(key(KeyCode::Enter)).unwrap();
            assert!(table.project_form.is_some());

            // Fill out name
            table.handle_key_event(key(KeyCode::Tab)).unwrap();
            table.handle_key_event(key(KeyCode::Char('a'))).unwrap();
            assert!(table.project_form.is_some());

            // Verify that saving the form closes it
            table.handle_key_event(key(KeyCode::Enter)).unwrap();
            assert!(table.project_form.is_none());
        }

        #[test]
        fn navigation() {
            let context = initialze_context();
            let mut table = ProjectTable::new(context.connection()).unwrap();

            assert!(table.get_selected_project().is_none());
            table.handle_key_event(key(KeyCode::Char('j'))).unwrap();
            assert!(table.get_selected_project().is_some());

            // Projects are ordered by name, so the order is:
            // id = 2
            // id = 3
            // id = 1

            // Navigate down with 'j'
            let event = table.handle_key_event(key(KeyCode::Char('j'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            let selected_project = table.get_selected_project().unwrap();
            assert_eq!(selected_project.id, 3);

            // Navigate up with 'k'
            let event = table.handle_key_event(key(KeyCode::Char('k'))).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            let selected_project = table.get_selected_project().unwrap();
            assert_eq!(selected_project.id, 2);

            // Navigate down with arrow down
            let event = table.handle_key_event(key(KeyCode::Down)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            let selected_project = table.get_selected_project().unwrap();
            assert_eq!(selected_project.id, 3);

            // Navigate up with arrow up
            let event = table.handle_key_event(key(KeyCode::Up)).unwrap();
            assert_eq!(event, KeyEventResult::Consumed);
            let selected_project = table.get_selected_project().unwrap();
            assert_eq!(selected_project.id, 2);
        }

        #[test]
        fn force_delete() {
            let context = initialze_context();
            let mut table = ProjectTable::new(context.connection()).unwrap();

            assert_eq!(table.projects.len(), 3);

            // Select first row
            table.handle_key_event(key(KeyCode::Char('j'))).unwrap();

            // Force delete with 'D' key
            table.handle_key_event(key(KeyCode::Char('D'))).unwrap();

            assert_eq!(table.projects.len(), 2);
            assert_eq!(table.projects.first().unwrap().id, 3);
            assert_eq!(table.projects.get(1).unwrap().id, 1);
        }

        #[test]
        fn unused_key() {
            let context = initialze_context();
            let mut table = ProjectTable::new(context.connection()).unwrap();

            let event = table.handle_key_event(key(KeyCode::Char('w'))).unwrap();
            assert_eq!(event, KeyEventResult::Unused);
        }
    }
}
