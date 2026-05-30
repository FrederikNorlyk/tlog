use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    /// Start time tracking on the given project
    Start {
        #[arg(long_help = "Id of the project to start tracking")]
        project_id: i32,
    },
    /// Stop time tracking of the give n project
    Stop {
        #[arg(long_help = "Id of the project to stop tracking")]
        project_id: i32,
    },
    /// List all sessions.
    #[command(visible_alias = "ls")]
    List {
        /// Only show duration for this project ID.
        #[arg(long = "project", short = 'p', long_help = "Filter by project id")]
        project_id: Option<i32>,

        /// Only show sessions for this date, in YYYY-MM-DD format.
        #[arg(long = "date", short = 'd', conflicts_with = "today")]
        date: Option<String>,

        /// Only show today's sessions
        #[arg(long, conflicts_with = "date")]
        today: bool,
    },
}
