use crate::core::app_error::AppError;
use crate::core::config::{Config, ConfigError, ConfigMetadata};
use crate::core::time_format::TimeFormat;
use crate::db::database::Database;
use crate::model::opener::Opener;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Where,
    /// Set time format (`Seconds` | `HoursMinutes` | `HoursMinutesSeconds` | `DecimalHours`)
    TimeFormat {
        value: Option<TimeFormat>,
    },
    Opener {
        #[arg(long)]
        url: Option<String>,
        #[arg(long, alias = "desc")]
        description: Option<String>,
    },
}

/// Handles various configuration commands, allowing the user to query or modify application
/// settings such as database paths, time formats, and URL openers.
///
/// # Errors
/// - Returns an error if the database connection has no associated path.
/// - Returns an error if the configuration file path cannot be created or accessed.
/// - Returns an error if the provided values for configuration updates are invalid.
///
pub fn handle_config_command(
    command: ConfigCommand,
    database: &Database,
    config: &ConfigMetadata,
) -> Result<(), AppError> {
    match command {
        ConfigCommand::Where => {
            let database_path = database
                .connection()
                .path()
                .ok_or_else(|| std::io::Error::other("Database connection has no path"))?;

            println!("Database: {database_path}");

            let config_path = Config::get_or_create_file_path()?;
            println!("Config: {}", config_path.display());
        }
        ConfigCommand::TimeFormat { value } => {
            if let Some(time_format) = value {
                Config::set_time_format(time_format)?;
            } else {
                println!("Time format: {:?}", config.time_format());
            }
        }
        ConfigCommand::Opener { url, description } => {
            if url.is_none() && description.is_none() {
                if let Some(opener) = config.opener() {
                    println!("{opener}");
                } else {
                    println!("No opener has been configured");
                }
                return Ok(());
            }

            let url = unwrap_or_get_existing_url(url, config.opener().as_ref())?;

            let description =
                unwrap_or_get_existing_description(description, config.opener().as_ref())?;

            let new_opener = Opener::new(url, description);
            Config::set_opener(Some(new_opener))?;
        }
    }

    Ok(())
}

fn unwrap_or_get_existing_url(
    url: Option<String>,
    opener: Option<&Opener>,
) -> Result<String, ConfigError> {
    if let Some(url) = url {
        Ok(url)
    } else if let Some(opener) = opener {
        Ok(opener.url_template().to_string())
    } else {
        // If no opener exists, both parameters are required
        Err(ConfigError::RequiredFieldMissing("url"))
    }
}

fn unwrap_or_get_existing_description(
    description: Option<String>,
    opener: Option<&Opener>,
) -> Result<String, ConfigError> {
    if let Some(description) = description {
        Ok(description)
    } else if let Some(opener) = opener {
        Ok(opener.description().to_string())
    } else {
        // If no opener exists, both parameters are required
        Err(ConfigError::RequiredFieldMissing("description"))
    }
}
