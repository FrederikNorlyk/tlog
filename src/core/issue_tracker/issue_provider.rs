use crate::core::app_error::AppError;
use crate::core::issue_tracker::issue::Issue;

pub trait IssueProvider {
    /// Fetches an issue using the implementing issue tracker.
    ///
    /// # Errors
    /// Returns an error if the issue tracker fails to fetch the issue
    fn fetch_issue(&self, id: &str) -> Result<Option<Issue>, AppError>;
}
