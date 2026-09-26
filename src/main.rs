use anyhow::Context;
use clap::Parser;
use std::process::ExitCode;
use time::OffsetDateTime;
use tlog::cli::commands::{Cli, Command};
use tlog::cli::config_command::handle_config_command;
use tlog::cli::project_command::handle_project_command;
use tlog::core::clipboard::system_clipboard::SystemClipboard;
use tlog::core::config::Config;
use tlog::core::diagnostic;
use tlog::core::time_format::TimeFormat;
use tlog::core::tracking::Tracking;
use tlog::db::database::Database;
use tlog::db::project_repository::ProjectRepository;
use tlog::model::session::Session;
use tlog::tui::terminal_user_interface::TerminalUserInterface;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {}", diagnostic::report(&error));
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let database = Database::new().context("Could not open application database")?;
    database.init().with_context(|| {
        format!(
            "Could not initialize database {}",
            database.connection().path().unwrap_or("<memory>")
        )
    })?;

    let config = Config::get().context("Could not load configuration")?;

    let cli = Cli::parse();

    let Some(command) = cli.command else {
        let clipboard = Box::new(SystemClipboard::new().context("Could not initialize clipboard")?);
        let mut tui = TerminalUserInterface::new(database.connection(), clipboard)
            .context("Could not initialize terminal interface")?;

        ratatui::run(|terminal| tui.run(terminal))?;

        return Ok(());
    };

    match command {
        Command::Project { command } => {
            let mut stdout = std::io::stdout();
            let project_repository = ProjectRepository::new(database.connection());
            handle_project_command(command, &project_repository, &mut stdout)
                .context("Could not execute project command")?;
        }
        Command::Start { project_id } => {
            let tracking = Tracking::new(database.connection());
            tracking
                .start(project_id)
                .with_context(|| format!("Could not start tracking project {project_id}"))?;
        }
        Command::Stop { project_id } => {
            let tracking = Tracking::new(database.connection());
            tracking
                .stop(project_id)
                .with_context(|| format!("Could not stop tracking project {project_id}"))?;
        }
        Command::Set {
            project_id,
            date,
            total_seconds,
        } => {
            let tracking = Tracking::new(database.connection());
            let query_date = date.unwrap_or_else(|| OffsetDateTime::now_utc().date());

            tracking
                .set(project_id, query_date, total_seconds)
                .with_context(|| {
                    format!("Could not set time for project {project_id} on {query_date}")
                })?;
        }
        Command::Reset { project_id, date } => {
            let tracking = Tracking::new(database.connection());
            tracking
                .reset(project_id, date)
                .with_context(|| format!("Could not reset project {project_id} on {date}"))?;
        }
        Command::List { date } => {
            const BOLD: &str = "\x1b[1m";
            const RESET: &str = "\x1b[0m";

            let tracking = Tracking::new(database.connection());
            let mut total = 0;
            let query_date = date.unwrap_or_else(|| OffsetDateTime::now_utc().date());
            let time_format = config.time_format();

            tracking
                .list_all_sessions(query_date, None)
                .with_context(|| format!("Could not list sessions on {query_date}"))?
                .iter()
                .for_each(|session| {
                    total += session.total_seconds;
                    print_session(session, time_format);
                });

            let duration = time_format.format(total);

            println!("{BOLD}{duration:10}      Total{RESET}");
        }
        Command::Config { command } => handle_config_command(command, &database, &config)?,
    }

    Ok(())
}

fn print_session(session: &Session, time_format: TimeFormat) {
    const LIGHT_GRAY: &str = "\x1b[90m";
    const BOLD: &str = "\x1b[1m";
    const RESET: &str = "\x1b[0m";

    let project = &session.project;
    let mut duration = time_format.format(session.total_seconds);

    if session.is_started {
        duration.push('*');
    }

    println!("{BOLD}{duration:10}{RESET}  {LIGHT_GRAY}{project}{RESET}");
}
