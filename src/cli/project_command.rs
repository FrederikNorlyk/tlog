use crate::db::project_repository::ProjectRepository;
use clap::Subcommand;
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
pub fn handle_project_command(
    command: ProjectCommand,
    project_repository: &ProjectRepository,
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
                    println!("{project:?}");
                } else {
                    println!("{project}");
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
    #[error("Project with id {project_id} was not found")]
    ProjectNotFound { project_id: i32 },
}
