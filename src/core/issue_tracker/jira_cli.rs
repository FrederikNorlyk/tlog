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
    pub fn fetch_issue(&self, name: &str) -> Result<Issue, AppError> {
        let output = Command::new("acli")
            .args(["jira", "workitem", "view", name, "-f", "description"])
            .output()
            .map_err(|e| AppError::Command(e.to_string()))?;

        if !output.status.success() {
            return Err(AppError::Command(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        let description = stdout
            .lines()
            .find_map(|line| line.strip_prefix("Description:"))
            .map(str::trim)
            .unwrap_or_default()
            .to_string();

        Ok(Issue {
            id: name.to_string(),
            description,
        })
    }
}
