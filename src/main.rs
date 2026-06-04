use clap::Parser;
use std::error::Error;
use time::OffsetDateTime;
use tlog::cli::commands::{Cli, Command};
use tlog::cli::config_command::ConfigCommand;
use tlog::cli::project_command::ProjectCommand;
use tlog::core::tracking::Tracking;
use tlog::db::database::Database;
use tlog::db::manual_session_repository::ManualSessionRepository;
use tlog::db::project_repository::ProjectRepository;
use tlog::tui::terminal_user_interface::TerminalUserInterface;
use tlog::util::format_util::FormatUtil;

fn main() -> Result<(), Box<dyn Error>> {
    let database = Database::new()?;
    database.init()?;

    let cli = Cli::parse();

    let Some(command) = cli.command else {
        let tui = TerminalUserInterface;
        ratatui::run(|terminal| tui.launch(terminal))?;
        return Ok(());
    };

    match command {
        Command::Project { command } => match command {
            ProjectCommand::Add { name, description } => {
                let project_repository = ProjectRepository::new(database.connection());
                let id = project_repository.insert(&name, description.as_deref())?;
                println!("Project #{id} created")
            }
            ProjectCommand::Update {
                id,
                name,
                description,
                clear_description,
            } => {
                let project_repository = ProjectRepository::new(database.connection());

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
                let project_repository = ProjectRepository::new(database.connection());

                if !project_repository.delete(id)? {
                    return Err(format!("Project with id {id} was not found").into());
                }
            }
            ProjectCommand::List { debug } => {
                let project_repository = ProjectRepository::new(database.connection());

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
        Command::Start { project_id } => {
            let tracking = Tracking::new(database.connection());
            tracking.start(project_id)?;
        }
        Command::Stop { project_id } => {
            let tracking = Tracking::new(database.connection());
            tracking.stop(project_id)?;
        }
        Command::Set {
            project_id,
            date,
            today,
            total_seconds,
        } => {
            let manual_session_repository = ManualSessionRepository::new(database.connection());

            let query_date = if today {
                OffsetDateTime::now_utc().date()
            } else {
                // clap ensures that date is never None if today is false
                date.expect("Missing date")
            };

            // TODO: Stop any running event
            manual_session_repository.upsert(project_id, query_date, total_seconds)?;
        }
        Command::Reset { project_id, date } => {
            let tracking = Tracking::new(database.connection());
            tracking.reset(project_id, date)?;
        }
        Command::List { date } => {
            let tracking = Tracking::new(database.connection());
            let mut total = 0;
            let query_date = date.unwrap_or_else(|| OffsetDateTime::now_utc().date());

            tracking
                .list_all_sessions(query_date)?
                .iter()
                .for_each(|session| {
                    total += session.total_seconds;
                    println!("{session}")});

            const BOLD: &str = "\x1b[1m";
            const RESET: &str = "\x1b[0m";

            let (hours, minutes, seconds) = FormatUtil::seconds_to_hms(total);

            println!("{BOLD}{hours:02}:{minutes:02}:{seconds:02}       Total{RESET}")
        }
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
