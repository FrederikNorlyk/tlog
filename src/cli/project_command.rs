use crate::core::app_error::AppError;
use crate::db::project_repository::ProjectRepository;
use clap::Subcommand;
use std::io::Write;
use thiserror::Error;

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// Add a new project.
    Add {
        /// Name of the project.
        name: String,

        /// Optional project description.
        #[arg(
            long = "desc",
            short = 'd',
            long_help = "Optional project description. If provided, it must not be empty or only whitespace."
        )]
        description: Option<String>,
    },
    /// Update an existing project by its ID.
    Update {
        /// ID of the project to update.
        id: i32,

        /// New project name.
        #[arg(
            long = "name",
            short = 'n',
            long_help = "New project name. If provided, it must not be empty or only whitespace."
        )]
        name: Option<String>,

        /// New project description.
        #[arg(
            long = "desc",
            short = 'd',
            conflicts_with = "clear_description",
            long_help = "New project description. If provided, it must not be empty or only whitespace."
        )]
        description: Option<String>,

        /// Clear the project description.
        #[arg(long = "clear-desc")]
        clear_description: bool,
    },
    /// Delete a project by its ID.
    #[command(visible_alias = "rm")]
    Delete {
        /// ID of the project to delete.
        id: i32,
    },
    /// List all projects.
    #[command(visible_alias = "ls")]
    List {
        #[arg(
            long = "debug",
            short = 'd',
            long_help = "Print projects using their Debug representation."
        )]
        debug: bool,
    },
}

/// Perform the matching action for the given command
///
/// # Errors
///
/// Returns an error if `SQLite` fails to execute a query,
/// or if the given command's arguments are invalid
pub fn handle_project_command<W: Write>(
    command: ProjectCommand,
    project_repository: &ProjectRepository,
    output: &mut W,
) -> Result<(), ProjectCommandError> {
    match command {
        ProjectCommand::Add { name, description } => {
            let id = project_repository.insert(&name, description.as_deref())?;
            println!("Project #{id} created");
        }
        ProjectCommand::Update {
            id,
            name,
            description,
            clear_description,
        } => {
            let Some(mut project) = project_repository.get(id)? else {
                return Err(ProjectCommandError::ProjectNotFound { project_id: id });
            };

            if let Some(name) = name {
                project.name = name;
            }

            if clear_description {
                project.description = None;
            } else if let Some(description) = description {
                project.description = Some(description);
            }

            project_repository.update(&project)?;
        }
        ProjectCommand::Delete { id } => {
            if !project_repository.delete(id)? {
                return Err(ProjectCommandError::ProjectNotFound { project_id: id });
            }
        }
        ProjectCommand::List { debug } => {
            project_repository.for_each(|project| {
                if debug {
                    writeln!(output, "{project:?}")?;
                } else {
                    writeln!(output, "{project}")?;
                }
                Ok(())
            })?;
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum ProjectCommandError {
    #[error("SQLITE error: {0}")]
    SQLite(#[from] rusqlite::Error),
    #[error("Application error: {0}")]
    AppErroor(#[from] AppError),
    #[error("Project with id {project_id} was not found")]
    ProjectNotFound { project_id: i32 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: ProjectCommand,
    }

    fn parse(args: &[&str]) -> ProjectCommand {
        TestCli::try_parse_from(args).unwrap().command
    }

    mod add {
        use super::*;

        #[test]
        fn no_description() {
            let command = parse(&["tlog", "add", "My Project"]);

            assert!(matches!(
                command,
                ProjectCommand::Add {
                    name,
                    description: _none
                } if name == "My Project"
            ));
        }

        #[test]
        fn with_description() {
            let command = parse(&["tlog", "add", "My Project", "--desc", "A description"]);

            assert!(matches!(
                command,
                ProjectCommand::Add {
                    name,
                    description: Some(description)
                } if name == "My Project"
                    && description == "A description"
            ));
        }
    }

    mod update {
        use super::*;

        #[test]
        fn update_name() {
            let command = parse(&["tlog", "update", "10", "--name", "Updated"]);

            assert!(matches!(
                command,
                ProjectCommand::Update {
                    id: 10,
                    name: Some(name),
                    description: None,
                    clear_description: false,
                } if name == "Updated"
            ));
        }

        #[test]
        fn clear_description() {
            let command = parse(&["tlog", "update", "10", "--clear-desc"]);

            assert!(matches!(
                command,
                ProjectCommand::Update {
                    id: 10,
                    clear_description: true,
                    ..
                }
            ));
        }

        #[test]
        fn description_and_clear_description_conflict() {
            let result = TestCli::try_parse_from([
                "tlog",
                "update",
                "10",
                "--desc",
                "new description",
                "--clear-desc",
            ]);

            assert!(result.is_err());
        }
    }

    mod delete {
        use super::*;

        #[test]
        fn delete_command() {
            let command = parse(&["tlog", "delete", "5"]);

            assert!(matches!(command, ProjectCommand::Delete { id: 5 }));
        }

        #[test]
        fn delete_alias() {
            let command = parse(&["tlog", "rm", "5"]);

            assert!(matches!(command, ProjectCommand::Delete { id: 5 }));
        }
    }

    mod list {
        use super::*;

        #[test]
        fn parses_list_command() {
            let command = parse(&["tlog", "list"]);

            assert!(matches!(command, ProjectCommand::List { debug: false }));
        }

        #[test]
        fn parses_list_debug_flag() {
            let command = parse(&["tlog", "list", "--debug"]);

            assert!(matches!(command, ProjectCommand::List { debug: true }));
        }
    }

    mod handle_project_command {
        use super::*;
        use crate::db::test_utils::DBTestContext;

        mod add {
            use super::*;

            #[test]
            fn creates_project() -> Result<(), Box<dyn std::error::Error>> {
                let context = DBTestContext::new()?;
                let repository = ProjectRepository::new(context.connection());

                handle_project_command(
                    ProjectCommand::Add {
                        name: "My Project".into(),
                        description: Some("Description".into()),
                    },
                    &repository,
                    &mut vec![],
                )?;

                let project = repository.get(1)?.expect("Project should exist");

                assert_eq!(project.id, 1);
                assert_eq!(project.name, "My Project");
                assert_eq!(project.description.as_deref(), Some("Description"));

                Ok(())
            }

            #[test]
            fn creates_project_without_description() -> Result<(), Box<dyn std::error::Error>> {
                let context = DBTestContext::new()?;
                let repository = ProjectRepository::new(context.connection());

                handle_project_command(
                    ProjectCommand::Add {
                        name: "My Project".into(),
                        description: None,
                    },
                    &repository,
                    &mut vec![],
                )?;

                let project = repository.get(1)?.expect("Project should exist");

                assert_eq!(project.name, "My Project");
                assert_eq!(project.description, None);

                Ok(())
            }
        }

        mod update {
            use super::*;

            #[test]
            fn updates_name() -> Result<(), Box<dyn std::error::Error>> {
                let context = DBTestContext::new()?;
                let repository = ProjectRepository::new(context.connection());

                repository.insert("Original", Some("Description"))?;

                handle_project_command(
                    ProjectCommand::Update {
                        id: 1,
                        name: Some("Updated".into()),
                        description: None,
                        clear_description: false,
                    },
                    &repository,
                    &mut vec![],
                )?;

                let project = repository.get(1)?.expect("Project should exist");

                assert_eq!(project.name, "Updated");
                assert_eq!(project.description.as_deref(), Some("Description"));

                Ok(())
            }

            #[test]
            fn updates_description() -> Result<(), Box<dyn std::error::Error>> {
                let context = DBTestContext::new()?;
                let repository = ProjectRepository::new(context.connection());

                repository.insert("Project", Some("Old"))?;

                handle_project_command(
                    ProjectCommand::Update {
                        id: 1,
                        name: None,
                        description: Some("New".into()),
                        clear_description: false,
                    },
                    &repository,
                    &mut vec![],
                )?;

                let project = repository.get(1)?.expect("Project should exist");

                assert_eq!(project.description.as_deref(), Some("New"));

                Ok(())
            }

            #[test]
            fn clears_description() -> Result<(), Box<dyn std::error::Error>> {
                let context = DBTestContext::new()?;
                let repository = ProjectRepository::new(context.connection());

                repository.insert("Project", Some("Description"))?;

                handle_project_command(
                    ProjectCommand::Update {
                        id: 1,
                        name: None,
                        description: None,
                        clear_description: true,
                    },
                    &repository,
                    &mut vec![],
                )?;

                let project = repository.get(1)?.expect("Project should exist");

                assert_eq!(project.description, None);

                Ok(())
            }

            #[test]
            fn updating_missing_project_fails() -> Result<(), Box<dyn std::error::Error>> {
                let context = DBTestContext::new()?;
                let repository = ProjectRepository::new(context.connection());

                let result = handle_project_command(
                    ProjectCommand::Update {
                        id: 999,
                        name: Some("Updated".into()),
                        description: None,
                        clear_description: false,
                    },
                    &repository,
                    &mut vec![],
                );

                assert!(matches!(
                    result,
                    Err(ProjectCommandError::ProjectNotFound { project_id: 999 })
                ));

                Ok(())
            }
        }

        mod delete {
            use super::*;

            #[test]
            fn deletes_existing_project() -> Result<(), Box<dyn std::error::Error>> {
                let context = DBTestContext::new()?;
                let repository = ProjectRepository::new(context.connection());

                repository.insert("Project", None)?;

                handle_project_command(ProjectCommand::Delete { id: 1 }, &repository, &mut vec![])?;

                assert!(repository.get(1)?.is_none());

                Ok(())
            }

            #[test]
            fn deleting_missing_project_fails() -> Result<(), Box<dyn std::error::Error>> {
                let context = DBTestContext::new()?;
                let repository = ProjectRepository::new(context.connection());

                let result = handle_project_command(
                    ProjectCommand::Delete { id: 999 },
                    &repository,
                    &mut vec![],
                );

                assert!(matches!(
                    result,
                    Err(ProjectCommandError::ProjectNotFound { project_id: 999 })
                ));

                Ok(())
            }
        }

        mod list {
            use super::*;
            use std::io::Cursor;

            fn run(
                cmd: ProjectCommand,
                repo: &ProjectRepository,
            ) -> Result<String, Box<dyn std::error::Error>> {
                let mut output = Cursor::new(Vec::new());

                handle_project_command(cmd, repo, &mut output)?;

                Ok(String::from_utf8(output.into_inner())?)
            }

            #[test]
            fn lists_projects() -> Result<(), Box<dyn std::error::Error>> {
                let context = DBTestContext::new()?;
                let repository = ProjectRepository::new(context.connection());

                repository.insert("Project A", None)?;
                repository.insert("Project B", Some("Description"))?;

                let output = run(ProjectCommand::List { debug: false }, &repository)?;

                let lines: Vec<&str> = output.lines().collect();

                assert_eq!(lines.len(), 2);

                assert_eq!(lines[0], "[90m 1[0m  [1mProject A[0m");
                assert_eq!(
                    lines[1],
                    "[90m 2[0m  [1mProject B[0m  [90mDescription[0m"
                );

                Ok(())
            }

            #[test]
            fn debug_format() -> Result<(), Box<dyn std::error::Error>> {
                let context = DBTestContext::new()?;
                let repository = ProjectRepository::new(context.connection());

                repository.insert("Project A", None)?;

                let output = run(ProjectCommand::List { debug: true }, &repository)?;

                let lines: Vec<&str> = output.lines().collect();

                assert_eq!(lines.len(), 1);

                let line = lines[0];

                assert_eq!(
                    line,
                    r#"Project { id: 1, name: "Project A", description: None }"#
                );

                Ok(())
            }
        }
    }
}
