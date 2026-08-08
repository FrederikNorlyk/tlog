use crate::core::app_error::AppError;
use crate::core::issue_tracker::issue::Issue;
use std::process::Command;

#[derive(Debug)]
pub struct JiraCLI;

impl JiraCLI {
    /// Fetches an issue using the Atlassian CLI.
    ///
    /// # Errors
    /// Returns an error if running the command fails.
    pub fn fetch_issue(query: &str, id_prefix: Option<&str>) -> Result<Option<Issue>, AppError> {
        let query = if let Some(prefix) = id_prefix {
            if query.starts_with(prefix) {
                query.to_owned()
            } else {
                format!("{prefix}{query}")
            }
        } else {
            query.to_owned()
        };

        let output = Command::new("acli")
            .args(["jira", "workitem", "view", query.as_str(), "-f", "summary"])
            .output()
            .map_err(|e| AppError::Command(e.to_string()))?;

        if !output.status.success() {
            return Err(AppError::Command(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        let id = stdout
            .lines()
            .find_map(|line| line.strip_prefix("Key:"))
            .map(str::trim)
            .unwrap_or_default()
            .to_string();

        let description = stdout
            .lines()
            .find_map(|line| line.strip_prefix("Summary:"))
            .map(str::trim)
            .unwrap_or_default()
            .to_string();

        Ok(Some(Issue { id, description }))
    }
}
