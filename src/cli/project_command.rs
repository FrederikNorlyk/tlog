use clap::Subcommand;

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
