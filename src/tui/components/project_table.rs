use crate::db::project_repository::ProjectRepository;
use crate::model::project::Project;
use crate::tui::components::alert_dialog::{AlertDialog, AlertDialogEvent};
use crate::tui::components::project_form::{ProjectForm, ProjectFormEvent};
use crate::tui::terminal_user_interface::TuiError::InvalidState;
use crate::tui::terminal_user_interface::{KeyEventResult, TuiError};
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
    pub fn new(connection: &'a Connection) -> rusqlite::Result<Self> {
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
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<KeyEventResult, TuiError> {
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
            };

            return Ok(KeyEventResult::Consumed);
        } else if let Some(form) = &mut self.project_form {
            match form.handle_key_event(key_event)? {
                ProjectFormEvent::Save { name, description } => {
                    self.insert_project(name, description)?;
                    self.project_form = None;
                }
                ProjectFormEvent::Cancel => {
                    self.project_form = None;
                }
                ProjectFormEvent::Consumed => {}
            };

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
                self.is_showing_deletion_alert_dialog = true
            }
            KeyCode::Char('D') if has_selected_project => self.delete_project()?,
            _ => did_match = false,
        }

        if did_match {
            return Ok(KeyEventResult::Consumed);
        }

        Ok(KeyEventResult::Unused)
    }

    fn delete_project(&mut self) -> Result<(), TuiError> {
        let Some(project) = self.get_selected_project() else {
            return Err(InvalidState {
                message: "No selected project",
            });
        };

        let project_repository = ProjectRepository::new(self.connection);
        project_repository.delete(project.id)?;
        self.refresh_projects()?;

        Ok(())
    }

    fn insert_project(
        &mut self,
        name: String,
        description: Option<String>,
    ) -> Result<(), TuiError> {
        let project_repository = ProjectRepository::new(self.connection);
        project_repository.insert(name.as_str(), description.as_deref())?;
        self.refresh_projects()?;

        Ok(())
    }

    fn refresh_projects(&mut self) -> Result<(), TuiError> {
        let project_repository = ProjectRepository::new(self.connection);

        self.projects.clear();

        project_repository.for_each(|project| {
            self.projects.push(project);
            Ok(())
        })?;

        Ok(())
    }

    fn get_selected_project(&self) -> Option<&Project> {
        let Some(selected_index) = self.state.selected() else {
            return None;
        };

        let session = &self.projects[selected_index];

        Some(session)
    }
}
