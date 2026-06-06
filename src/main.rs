use clap::Parser;
use std::error::Error;
use time::OffsetDateTime;
use tlog::cli::commands::{Cli, Command};
use tlog::cli::config_command::ConfigCommand;
use tlog::cli::project_command::handle_project_command;
use tlog::core::tracking::Tracking;
use tlog::db::database::Database;
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
        Command::Project { command } => {
            let project_repository = ProjectRepository::new(database.connection());
            handle_project_command(command, &project_repository)?;
        }
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
            total_seconds,
        } => {
            let tracking = Tracking::new(database.connection());
            let query_date = date.unwrap_or_else(|| OffsetDateTime::now_utc().date());

            tracking.set(project_id, query_date, total_seconds)?;
        }
        Command::Reset { project_id, date } => {
            let tracking = Tracking::new(database.connection());
            tracking.reset(project_id, date)?;
        }
        Command::List { date } => {
            const BOLD: &str = "\x1b[1m";
            const RESET: &str = "\x1b[0m";

            let tracking = Tracking::new(database.connection());
            let mut total = 0;
            let query_date = date.unwrap_or_else(|| OffsetDateTime::now_utc().date());

            tracking
                .list_all_sessions(query_date)?
                .iter()
                .for_each(|session| {
                    total += session.total_seconds;
                    println!("{session}");
                });

            let (hours, minutes, seconds) = FormatUtil::seconds_to_hms(total);

            println!("{BOLD}{hours:02}:{minutes:02}:{seconds:02}       Total{RESET}");
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
