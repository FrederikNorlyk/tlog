use crate::core::app_error::AppError;
use crate::core::issue_tracker::issue::Issue;
use crate::core::issue_tracker::issue_provider::IssueProvider;
use crate::model::project::Project;
use crate::tui::components::dialog::Dialog;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Color, Widget};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui_textarea::TextArea;
use std::sync::Arc;

pub struct IssueFinderForm<'a> {
    id_text_area: TextArea<'a>,
    error_text: Option<String>,
    issue_provider: Arc<dyn IssueProvider>,
}

impl IssueFinderForm<'_> {
    #[must_use]
    pub fn new(issue_provider: Arc<dyn IssueProvider>) -> Self {
        let mut id_text_area = TextArea::new(vec![]);

        id_text_area.set_style(Style::default().fg(Color::DarkGray));

        id_text_area.set_block(
            Block::default()
                .border_style(Color::DarkGray)
                .borders(Borders::ALL)
                .title("Issue ID"),
        );

        Self {
            id_text_area,
            error_text: None,
            issue_provider,
        }
    }

    /// Handle any key event from the user.
    ///
    /// # Errors
    /// Returns an error if finding the issue using the configured issue tracker fails.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> IssueFinderEvent {
        match key_event.code {
            KeyCode::Esc => return IssueFinderEvent::Cancel,
            KeyCode::Enter => {
                let is_valid = self.validate_form();

                if !is_valid {
                    return IssueFinderEvent::Consumed;
                }

                match self.find_issue() {
                    Ok(Some(issue)) => {
                        return IssueFinderEvent::ProjectFound {
                            project: issue.into(),
                        };
                    }
                    Ok(None) => self.error_text = Some("Could not find the issue".to_string()),
                    Err(e) => self.error_text = Some(e.to_string()),
                }
            }
            _ => {
                self.id_text_area.input(key_event);
            }
        }

        IssueFinderEvent::Consumed
    }

    fn validate_form(&mut self) -> bool {
        let mut is_valid = true;

        let Some(block) = self.id_text_area.block().cloned() else {
            return false;
        };

        if self.get_field_value().is_empty() {
            is_valid = false;
            self.id_text_area.set_block(block.border_style(Color::Red));
        } else {
            self.id_text_area
                .set_block(block.border_style(Color::DarkGray));
        }

        is_valid
    }

    fn get_field_value(&self) -> String {
        self.id_text_area
            .lines()
            .first()
            .map_or("", String::as_str)
            .trim()
            .to_string()
    }

    fn find_issue(&self) -> Result<Option<Issue>, AppError> {
        let issue_id = self.get_field_value();

        let Some(issue) = self.issue_provider.fetch_issue(issue_id.as_str())? else {
            return Ok(None);
        };

        Ok(Some(issue))
    }
}

impl Widget for &IssueFinderForm<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog = Dialog::constrained(Constraint::Percentage(90), Constraint::Length(10));
        let inner = dialog.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Fill(1)])
            .split(inner);

        self.id_text_area.render(chunks[0], buf);

        if let Some(error) = self.error_text.as_ref() {
            Paragraph::new(error.as_str()).render(chunks[1], buf);
        }
    }
}

pub enum IssueFinderEvent {
    ProjectFound { project: Project },
    Cancel,
    Consumed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    struct MockIssueTracker {
        issue: Option<Issue>,
    }

    impl IssueProvider for MockIssueTracker {
        fn fetch_issue(&self, _id: &str) -> Result<Option<Issue>, AppError> {
            Ok(self.issue.clone())
        }
    }

    fn create_form(issue: Option<Issue>) -> IssueFinderForm<'static> {
        IssueFinderForm::new(Arc::new(MockIssueTracker { issue }))
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn validate_form() {
        let mut form = create_form(None);

        assert!(!form.validate_form());

        form.id_text_area.insert_str("TEST-123");

        assert!(form.validate_form());
    }

    #[test]
    fn get_field_value() {
        let mut form = create_form(None);

        form.id_text_area.insert_str("  TEST-123  ");

        assert_eq!(form.get_field_value(), "TEST-123");
    }

    #[test]
    fn find_issue() {
        let issue = Issue {
            id: "TEST-123".to_string(),
            description: "Test issue".to_string(),
        };

        let form = create_form(Some(issue.clone()));

        let result = form.find_issue();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(issue));
    }

    mod handle_key_event {
        use super::*;

        #[test]
        fn escape_closes() {
            let mut form = create_form(None);
            let event = form.handle_key_event(key(KeyCode::Esc));

            assert!(matches!(event, IssueFinderEvent::Cancel));
        }

        #[test]
        fn type_into_text_area() {
            let mut form = create_form(None);
            let event = form.handle_key_event(key(KeyCode::Char('A')));

            assert!(matches!(event, IssueFinderEvent::Consumed));
            assert_eq!(form.get_field_value(), "A");
        }

        mod enter_submits {
            use super::*;

            #[test]
            fn empty_text_area_is_invalid() {
                let mut form = create_form(None);
                let event = form.handle_key_event(key(KeyCode::Enter));

                assert!(matches!(event, IssueFinderEvent::Consumed));
                assert!(form.error_text.is_none());
            }

            #[test]
            fn issue_not_found() {
                let mut form = create_form(None);

                form.id_text_area.insert_str("TEST-123");

                let event = form.handle_key_event(key(KeyCode::Enter));

                assert!(matches!(event, IssueFinderEvent::Consumed));

                assert_eq!(
                    form.error_text,
                    Some("Could not find the issue".to_string())
                );
            }

            #[test]
            fn issue_found() {
                let issue = Issue {
                    id: "TEST-123".to_string(),
                    description: "Fix bug".to_string(),
                };

                let mut form = create_form(Some(issue));

                form.id_text_area.insert_str("TEST-123");

                let event = form.handle_key_event(key(KeyCode::Enter));

                assert!(matches!(event, IssueFinderEvent::ProjectFound { .. }));
            }
        }
    }

    mod render {
        use super::*;
        use crate::tui::render_test_util::RenderTestUtil;

        #[test]
        fn empty() {
            let form = create_form(None);

            let area = Rect::new(0, 0, 60, 10);
            let mut buf = Buffer::empty(area);

            form.render(area, &mut buf);

            let expected = vec![
                "   ┌─────────────────────────────────────────────── Esc ┐   ",
                "   │┌Issue ID──────────────────────────────────────────┐│   ",
                "   ││                                                  ││   ",
                "   │└──────────────────────────────────────────────────┘│   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   └────────────────────────────────────────────────────┘   ",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn with_text() {
            let mut form = create_form(None);

            form.id_text_area.insert_str("PROJ-123");

            let area = Rect::new(0, 0, 60, 10);
            let mut buf = Buffer::empty(area);

            form.render(area, &mut buf);

            let expected = vec![
                "   ┌─────────────────────────────────────────────── Esc ┐   ",
                "   │┌Issue ID──────────────────────────────────────────┐│   ",
                "   ││PROJ-123                                          ││   ",
                "   │└──────────────────────────────────────────────────┘│   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   └────────────────────────────────────────────────────┘   ",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }

        #[test]
        fn error_message() {
            let mut form = create_form(None);

            form.error_text = Some("Could not find the issue".to_string());

            let area = Rect::new(0, 0, 60, 10);
            let mut buf = Buffer::empty(area);

            form.render(area, &mut buf);

            let expected = vec![
                "   ┌─────────────────────────────────────────────── Esc ┐   ",
                "   │┌Issue ID──────────────────────────────────────────┐│   ",
                "   ││                                                  ││   ",
                "   │└──────────────────────────────────────────────────┘│   ",
                "   │Could not find the issue                            │   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   │                                                    │   ",
                "   └────────────────────────────────────────────────────┘   ",
            ];

            RenderTestUtil::assert_eq(expected, &buf);
        }
    }
}
