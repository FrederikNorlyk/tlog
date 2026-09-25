pub mod issue;
pub mod issue_provider;
pub mod jira_cli;

use crate::core::app_error::AppError;
use crate::core::issue_tracker::issue::Issue;
use crate::core::issue_tracker::issue_provider::IssueProvider;
use crate::core::issue_tracker::jira_cli::JiraCLI;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum IssueTracker {
    Jira { id_prefix: Option<String> },
}

impl IssueProvider for IssueTracker {
    fn fetch_issue(&self, id: &str) -> Result<Option<Issue>, AppError> {
        match self {
            Self::Jira { id_prefix } => JiraCLI::fetch_issue(id, id_prefix.as_deref()),
        }
    }
}
