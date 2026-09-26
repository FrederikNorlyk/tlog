use crate::core::issue_tracker::issue::Issue;
use std::process::{Command, ExitStatus};
use thiserror::Error;

#[derive(Debug)]
pub struct JiraCLI;

impl JiraCLI {
    /// Fetches an issue using the Atlassian CLI.
    ///
    /// # Errors
    /// Returns an error if running the command fails.
    pub fn fetch_issue(query: &str, id_prefix: Option<&str>) -> Result<Option<Issue>, JiraError> {
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
            .map_err(|source| JiraError::Launch {
                query: query.clone(),
                source,
            })?;

        if !output.status.success() {
            return Err(JiraError::Failed {
                query,
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
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

#[derive(Debug, Error)]
pub enum JiraError {
    #[error("Could not launch acli for Jira issue {query}")]
    Launch {
        query: String,
        #[source]
        source: std::io::Error,
    },
    #[error("acli failed for Jira issue {query} ({status})\n{stderr}")]
    Failed {
        query: String,
        status: ExitStatus,
        stderr: String,
    },
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    struct TestContext {
        directory: tempfile::TempDir,
        previous_path: Option<OsString>,
    }

    impl TestContext {
        // All callers hold the shared serial_test lock while PATH is overridden.
        #[allow(unsafe_code)]
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let previous_path = std::env::var_os("PATH");
            unsafe {
                std::env::set_var("PATH", directory.path());
            }
            Self {
                directory,
                previous_path,
            }
        }

        fn install_acli(&self, stdout: &[u8], stderr: &[u8], exit_code: u8) {
            fs::write(self.directory.path().join("stdout"), stdout).unwrap();
            fs::write(self.directory.path().join("stderr"), stderr).unwrap();
            let executable = self.directory.path().join("acli");
            fs::write(
                &executable,
                format!(
                    "#!/bin/sh\n\
                     directory=${{0%/*}}\n\
                     printf '%s\\n' \"$@\" > \"$directory/args\"\n\
                     /bin/cat \"$directory/stdout\"\n\
                     /bin/cat \"$directory/stderr\" >&2\n\
                     exit {exit_code}\n"
                ),
            )
            .unwrap();
            fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
        }

        fn args(&self) -> Vec<String> {
            fs::read_to_string(self.directory.path().join("args"))
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect()
        }
    }

    impl Drop for TestContext {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            unsafe {
                if let Some(previous) = &self.previous_path {
                    std::env::set_var("PATH", previous);
                } else {
                    std::env::remove_var("PATH");
                }
            }
        }
    }

    mod fetch_issue {
        use super::*;

        mod arguments {
            use super::*;

            fn assert_query(query: &str, prefix: Option<&str>, expected: &str) {
                let context = TestContext::new();
                context.install_acli(b"Key: PROJ-42\nSummary: Fix login\n", b"", 0);

                JiraCLI::fetch_issue(query, prefix).unwrap();

                assert_eq!(
                    context.args(),
                    ["jira", "workitem", "view", expected, "-f", "summary"]
                );
            }

            #[test]
            #[serial]
            fn without_prefix() {
                assert_query("PROJ-42", None, "PROJ-42");
            }

            #[test]
            #[serial]
            fn adds_missing_prefix() {
                assert_query("42", Some("PROJ-"), "PROJ-42");
            }

            #[test]
            #[serial]
            fn keeps_existing_prefix() {
                assert_query("PROJ-42", Some("PROJ-"), "PROJ-42");
            }

            #[test]
            #[serial]
            fn prefix_must_be_at_start() {
                assert_query("OTHER-PROJ-42", Some("PROJ-"), "PROJ-OTHER-PROJ-42");
            }

            #[test]
            #[serial]
            fn empty_prefix_keeps_query() {
                assert_query("PROJ-42", Some(""), "PROJ-42");
            }

            #[test]
            #[serial]
            fn passes_query_as_one_literal_argument() {
                assert_query("PROJ-42 ; $(echo test)", None, "PROJ-42 ; $(echo test)");
            }
        }

        mod output {
            use super::*;

            fn fetch(stdout: &[u8]) -> Issue {
                let context = TestContext::new();
                context.install_acli(stdout, b"", 0);

                JiraCLI::fetch_issue("42", Some("PROJ-"))
                    .unwrap()
                    .expect("Successful output should produce an issue")
            }

            #[test]
            #[serial]
            fn parses_key_and_summary() {
                let issue = fetch(b"Key: PROJ-42\nSummary: Fix login\n");

                assert_eq!(issue, Issue::new("PROJ-42".into(), "Fix login".into()));
            }

            #[test]
            #[serial]
            fn trims_fields_and_ignores_unrelated_lines() {
                let issue = fetch(
                    b"Status: Open\r\nSummary: \tFix login: handle spaces  \r\nKey:  PROJ-42 \t\r\n",
                );

                assert_eq!(issue.id, "PROJ-42");
                assert_eq!(issue.description, "Fix login: handle spaces");
            }

            #[test]
            #[serial]
            fn uses_first_matching_fields() {
                let issue = fetch(b"Key: PROJ-42\nKey: OTHER-1\nSummary: First\nSummary: Second\n");

                assert_eq!(issue.id, "PROJ-42");
                assert_eq!(issue.description, "First");
            }

            #[test]
            #[serial]
            fn missing_fields_default_to_empty_strings() {
                for (stdout, id, description) in [
                    ("Summary: Fix login\n", "", "Fix login"),
                    ("Key: PROJ-42\n", "PROJ-42", ""),
                    ("", "", ""),
                ] {
                    let issue = fetch(stdout.as_bytes());

                    assert_eq!(issue, Issue::new(id.into(), description.into()));
                }
            }

            #[test]
            #[serial]
            fn replaces_invalid_utf8() {
                let issue = fetch(b"Key: PROJ-42\nSummary: Fix \xff\n");

                assert_eq!(issue.description, "Fix \u{fffd}");
            }

            #[test]
            #[serial]
            fn successful_command_can_write_to_stderr() {
                let context = TestContext::new();
                context.install_acli(b"Key: PROJ-42\nSummary: Fix login\n", b"Warning\n", 0);

                let issue = JiraCLI::fetch_issue("42", Some("PROJ-")).unwrap().unwrap();

                assert_eq!(issue, Issue::new("PROJ-42".into(), "Fix login".into()));
            }
        }

        mod errors {
            use super::*;

            #[test]
            #[serial]
            fn missing_executable() {
                let _context = TestContext::new();

                let result = JiraCLI::fetch_issue("PROJ-42", None);

                let error = result.unwrap_err();
                let source = std::error::Error::source(&error)
                    .expect("launch error must preserve its source");
                assert_eq!(
                    source.downcast_ref::<std::io::Error>().unwrap().kind(),
                    std::io::ErrorKind::NotFound
                );
            }

            #[test]
            #[serial]
            fn failed_command_returns_stderr_even_with_valid_stdout() {
                let context = TestContext::new();
                context.install_acli(
                    b"Key: PROJ-42\nSummary: Fix login\n",
                    b"Not authenticated\n",
                    1,
                );

                let result = JiraCLI::fetch_issue("PROJ-42", None);

                assert!(matches!(result,
                    Err(JiraError::Failed { status, stderr, .. }) if status.code() == Some(1) && stderr == "Not authenticated\n"
                ));
            }

            #[test]
            #[serial]
            fn permission_denied_preserves_os_error() {
                let context = TestContext::new();
                context.install_acli(b"", b"", 0);
                fs::set_permissions(
                    context.directory.path().join("acli"),
                    fs::Permissions::from_mode(0o644),
                )
                .unwrap();
                let error = JiraCLI::fetch_issue("PROJ-42", None).unwrap_err();
                assert!(
                    matches!(error, JiraError::Launch { source, .. } if source.kind() == std::io::ErrorKind::PermissionDenied)
                );
            }

            #[test]
            #[serial]
            fn signal_termination_keeps_status_and_stderr() {
                use std::os::unix::process::ExitStatusExt;
                let context = TestContext::new();
                context.install_acli(b"", b"", 0);
                fs::write(
                    context.directory.path().join("acli"),
                    "#!/bin/sh\nprintf 'interrupted request' >&2\nkill -TERM $$\n",
                )
                .unwrap();
                let error = JiraCLI::fetch_issue("PROJ-42", None).unwrap_err();
                assert!(
                    matches!(&error, JiraError::Failed { status, stderr, .. } if status.code().is_none() && status.signal() == Some(15) && stderr == "interrupted request")
                );
                let report = crate::core::diagnostic::report(&error.into());
                assert!(report.contains("signal"), "{report}");
                assert!(report.contains("PROJ-42"), "{report}");
                assert_eq!(report.matches("interrupted request").count(), 1);
            }

            #[test]
            #[serial]
            fn failed_command_replaces_invalid_utf8_in_stderr() {
                let context = TestContext::new();
                context.install_acli(b"", b"Error: \xff", 1);

                let result = JiraCLI::fetch_issue("PROJ-42", None);

                assert!(matches!(result,
                    Err(JiraError::Failed { status, stderr, .. }) if status.code() == Some(1) && stderr == "Error: \u{fffd}"
                ));
            }
        }
    }
}
