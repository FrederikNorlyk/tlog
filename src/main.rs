use clap::Parser;
use std::error::Error;
use time::macros::format_description;
use time::OffsetDateTime;
use tlog::cli::commands::{Cli, Command};
use tlog::cli::config_command::ConfigCommand;
use tlog::cli::project_command::ProjectCommand;
use tlog::cli::session_command::SessionCommand;
use tlog::core::tracking::Tracking;
use tlog::db::database::{Database, Repository};
use tlog::db::event_repository::EventRepository;
use tlog::db::project_repository::ProjectRepository;
use tlog::tui::app::TerminalUserInterface;

fn main() -> Result<(), Box<dyn Error>> {
    let database = Database::new()?;
    let project_repository = ProjectRepository::new(database.connection());
    let event_repository = EventRepository::new(database.connection());

    project_repository.initialize_schema()?;
    event_repository.initialize_schema()?;

    let cli = Cli::parse();

    let Some(command) = cli.command else {
        let tui = TerminalUserInterface::default();
        ratatui::run(|terminal| tui.launch(terminal))?;
        return Ok(());
    };

    match command {
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
                    Ok(())
                })?;
            }
        },
        Command::Session { command } => match command {
            SessionCommand::Start { project_id } => {
                let tracking = Tracking::new(database.connection());
                tracking.start(project_id)?;
            }
            SessionCommand::Stop { project_id } => {
                let tracking = Tracking::new(database.connection());
                tracking.stop(project_id)?;
            }
            SessionCommand::List {
                project_id,
                date,
                today,
            } => {
                let query_date: Option<String>;
                if today {
                    let now = OffsetDateTime::now_utc();

                    let format = format_description!("[year]-[month]-[day]");
                    let date_str = now.format(&format)?;

                    query_date = Some(date_str);
                } else {
                    query_date = date;
                }

                event_repository.for_each_session(
                    project_id,
                    query_date.as_deref(),
                    |session| {
                        println!("{session}");
                        Ok(())
                    },
                )?;
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
