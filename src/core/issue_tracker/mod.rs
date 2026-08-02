pub mod issue;
pub mod jira_cli;

use crate::core::app_error::AppError;
use crate::core::issue_tracker::issue::Issue;

pub enum IssueTracker {
    Jira(jira_cli::JiraCLI),
}

impl IssueTracker {
    /// Fetches an issue using the configured issue tracker.
    ///
    /// # Errors
    /// Returns an error if the issue tracker fails to fetch the issue
    pub fn fetch_issue(&self, name: &str) -> Result<Issue, AppError> {
        match self {
            Self::Jira(config) => config.fetch_issue(name),
        }
    }
}
