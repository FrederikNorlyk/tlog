use clap::Parser;
use std::error::Error;
use tlog::cli::commands::{Cli, Command};
use tlog::cli::config_command::ConfigCommand;
use tlog::cli::project_command::ProjectCommand;
use tlog::db::database::{Database, Repository};
use tlog::db::project_repository::ProjectRepository;

fn main() -> Result<(), Box<dyn Error>> {
    let database = Database::new()?;
    let project_repository = ProjectRepository::new(database.connection());

    project_repository.initialize_schema()?;

    let cli = Cli::parse();

    match cli.command {
        Command::Project { command } => match command {
            ProjectCommand::Add { name, description } => {
                project_repository.insert(&name, description.as_deref())?;
            }
            ProjectCommand::Update {
                id,
                name,
                description,
                clear_description,
            } => {
                let Some(mut project) = project_repository.get(id)? else {
                    return Err(format!("Project with id {id} was not found").into());
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
                    return Err(format!("Project with id {id} was not found").into());
                }
            }
            ProjectCommand::List { debug } => {
                project_repository.for_each(|project| {
                    if debug {
                        println!("{project:?}");
                    } else {
                        println!("{project}");
                    }
                })?;
            }
        },
        Command::Config { command } => match command {
            ConfigCommand::Where => {
                let path = database
                    .connection()
                    .path()
                    .ok_or_else(|| std::io::Error::other("Database connection has no path"))?;

                println!("{path}");
            }
        },
    }

    Ok(())
}
