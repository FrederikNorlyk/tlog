use clap::{Parser, Subcommand};
use crate::cli::config_command::ConfigCommand;
use crate::cli::project_command::ProjectCommand;

#[derive(Debug, Parser)]
#[command(
    name = "tlog",
    version,
    about = "Track time spent on projects",
    long_about = "tlog is a small command-line time tracker for managing projects and logging work.",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage projects.
    #[command(visible_alias = "p")]
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Inspect application configuration and storage paths.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

