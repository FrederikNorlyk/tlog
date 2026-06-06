use crate::cli::config_command::ConfigCommand;
use crate::cli::project_command::ProjectCommand;
use clap::{Parser, Subcommand};
use time::error::Parse;
use time::format_description::well_known::Iso8601;
use time::Date;

#[derive(Debug, Parser)]
#[command(
    name = "tlog",
    version,
    about = "Track time spent on projects",
    long_about = "tlog is a small command-line time tracker for managing projects and logging work."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage projects.
    #[command(visible_alias = "p")]
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Start time tracking on the given project
    Start {
        /// Id of the project to start tracking
        #[arg(long = "project", short = 'p')]
        project_id: i32,
    },
    /// Stop time tracking of the given project
    Stop {
        /// Id of the project to stop tracking
        #[arg(long = "project", short = 'p')]
        project_id: i32,
    },
    /// Manually set time spent on a project
    Set {
        /// ID of the project to update
        #[arg(long = "project", short = 'p')]
        project_id: i32,

        /// Date in YYYY-MM-DD format
        #[arg(long, short = 'd', value_parser = parse_date)]
        date: Option<Date>,

        /// Time spent on the project, in the hh:mm format
        #[arg(long = "duration", value_parser = parse_duration)]
        total_seconds: i64,
    },
    /// Reset all time tracking of the given project on the given date
    Reset {
        /// Id of the project to reset
        #[arg(long = "project", short = 'p')]
        project_id: i32,

        /// Date in YYYY-MM-DD format
        #[arg(value_parser = parse_date)]
        date: Date,
    },
    /// List all sessions.
    #[command(visible_alias = "ls")]
    List {
        /// Only show sessions for this date, in YYYY-MM-DD format.
        #[arg(long, short = 'd', value_parser = parse_date)]
        date: Option<Date>,
    },
    /// Inspect application configuration and storage paths.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

fn parse_date(s: &str) -> Result<Date, Parse> {
    Date::parse(s, &Iso8601::DATE)
}

fn parse_duration(s: &str) -> Result<i64, String> {
    let (h, m) = s.split_once(':').ok_or("expected hh:mm format")?;

    let hours: i64 = h.parse().map_err(|_| "invalid hours")?;
    let minutes: i64 = m.parse().map_err(|_| "invalid minutes")?;

    if minutes >= 60 {
        return Err("minutes must be < 60".into());
    }

    Ok(hours * 3600 + minutes * 60)
}

