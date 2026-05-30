use crate::cli::config_command::ConfigCommand;
use crate::cli::project_command::ProjectCommand;
use clap::{Parser, Subcommand};
use crate::cli::session_command::SessionCommand;

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
    /// Manage time tracking sessions.
    #[command(visible_alias = "s")]
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// Inspect application configuration and storage paths.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}
