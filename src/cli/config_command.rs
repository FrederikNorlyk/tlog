use crate::core::config::{Config, ConfigError, ConfigMetadata};
use crate::core::time_format::TimeFormat;
use crate::db::database::Database;
use crate::model::opener::Opener;
use anyhow::Context;
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
        #[arg(long)]
        name: Option<String>,
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
) -> anyhow::Result<()> {
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
                Config::set_time_format(time_format).context("Could not change time format")?;
            } else {
                println!("Time format: {:?}", config.time_format());
            }
        }
        ConfigCommand::Opener { url, name } => {
            if url.is_none() && name.is_none() {
                if let Some(opener) = config.opener() {
                    println!("{opener}");
                } else {
                    println!("No opener has been configured");
                }
                return Ok(());
            }

            let url = unwrap_or_get_existing_url(url, config.opener().as_ref())
                .context("Could not configure opener")?;

            let name = unwrap_or_get_existing_name(name, config.opener().as_ref())
                .context("Could not configure opener")?;

            let new_opener = Opener::new(url, name);
            Config::set_opener(Some(new_opener)).context("Could not save opener")?;
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

fn unwrap_or_get_existing_name(
    name: Option<String>,
    opener: Option<&Opener>,
) -> Result<String, ConfigError> {
    if let Some(name) = name {
        Ok(name)
    } else if let Some(opener) = opener {
        Ok(opener.name().to_string())
    } else {
        // If no opener exists, both parameters are required
        Err(ConfigError::RequiredFieldMissing("name"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: ConfigCommand,
    }

    fn parse(args: &[&str]) -> ConfigCommand {
        TestCli::try_parse_from(args).unwrap().command
    }

    mod parsing {
        use super::*;

        #[test]
        fn where_command() {
            assert!(matches!(parse(&["tlog", "where"]), ConfigCommand::Where));
        }

        mod time_format {
            use super::*;

            #[test]
            fn without_value() {
                assert!(matches!(
                    parse(&["tlog", "time-format"]),
                    ConfigCommand::TimeFormat { value: None }
                ));
            }

            #[test]
            fn supported_values() {
                for (argument, expected) in [
                    ("seconds", TimeFormat::Seconds),
                    ("hours-minutes", TimeFormat::HoursMinutes),
                    ("hours-minutes-seconds", TimeFormat::HoursMinutesSeconds),
                    ("decimal-hours", TimeFormat::DecimalHours),
                ] {
                    assert!(matches!(
                        parse(&["tlog", "time-format", argument]),
                        ConfigCommand::TimeFormat { value: Some(actual) } if actual == expected
                    ));
                }
            }

            #[test]
            fn rejects_unknown_value() {
                let error =
                    TestCli::try_parse_from(["tlog", "time-format", "minutes"]).unwrap_err();

                assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
            }
        }

        mod opener {
            use super::*;

            #[test]
            fn without_options() {
                assert!(matches!(
                    parse(&["tlog", "opener"]),
                    ConfigCommand::Opener {
                        url: None,
                        name: None
                    }
                ));
            }

            #[test]
            fn url_only() {
                assert!(matches!(
                    parse(&["tlog", "opener", "--url", "https://example.com/%s"]),
                    ConfigCommand::Opener { url: Some(url), name: None }
                        if url == "https://example.com/%s"
                ));
            }

            #[test]
            fn name() {
                assert!(matches!(
                    parse(&["tlog", "opener", "--name", "Test"]),
                    ConfigCommand::Opener { url: None, name: Some(name) }
                        if name == "Test"
                ));
            }

            #[test]
            fn both_options() {
                assert!(matches!(
                    parse(&["tlog", "opener", "--url", "https://example.com/%s", "--name", "Some name"]),
                    ConfigCommand::Opener { url: Some(url), name: Some(name) }
                        if url == "https://example.com/%s" && name == "Some name"
                ));
            }
        }
    }

    mod handle_config_command {
        use super::*;
        use crate::core::constants::CONFIG_DIR_ENV;
        use serial_test::serial;
        use std::ffi::OsString;

        struct TestContext {
            directory: tempfile::TempDir,
            previous_config_dir: Option<OsString>,
            database: Database,
        }

        impl TestContext {
            // Call only from tests holding the shared serial_test lock, like the
            // existing config and paths tests that modify this environment variable.
            #[allow(unsafe_code)]
            fn new() -> Self {
                let directory = tempfile::tempdir().unwrap();
                let database = Database::new_in_memory_db().unwrap();
                let previous_config_dir = std::env::var_os(CONFIG_DIR_ENV);
                unsafe {
                    std::env::set_var(CONFIG_DIR_ENV, directory.path());
                }
                Self {
                    directory,
                    previous_config_dir,
                    database,
                }
            }

            fn run(&self, command: ConfigCommand) -> anyhow::Result<()> {
                handle_config_command(command, &self.database, &Config::get()?)
            }

            fn contents(&self) -> String {
                std::fs::read_to_string(self.directory.path().join("tlog.toml")).unwrap()
            }
        }

        impl Drop for TestContext {
            #[allow(unsafe_code)]
            fn drop(&mut self) {
                unsafe {
                    if let Some(previous) = &self.previous_config_dir {
                        std::env::set_var(CONFIG_DIR_ENV, previous);
                    } else {
                        std::env::remove_var(CONFIG_DIR_ENV);
                    }
                }
            }
        }

        mod where_command {
            use super::*;

            #[test]
            #[serial]
            fn creates_missing_configuration() {
                let context = TestContext::new();
                let path = context.directory.path().join("tlog.toml");
                assert!(!path.exists());

                handle_config_command(
                    ConfigCommand::Where,
                    &context.database,
                    &ConfigMetadata::default(),
                )
                .unwrap();

                assert!(path.is_file());
                assert_eq!(
                    Config::get().unwrap().time_format(),
                    TimeFormat::HoursMinutesSeconds
                );
            }
        }

        mod time_format {
            use super::*;

            #[test]
            #[serial]
            fn saves_value_and_preserves_opener() {
                let context = TestContext::new();
                Config::set_opener(Some(Opener::new("https://example.com/%s", "Example"))).unwrap();

                context
                    .run(ConfigCommand::TimeFormat {
                        value: Some(TimeFormat::DecimalHours),
                    })
                    .unwrap();

                let config = Config::get().unwrap();
                assert_eq!(config.time_format(), TimeFormat::DecimalHours);
                let opener = config.opener().as_ref().unwrap();
                assert_eq!(opener.url_template(), "https://example.com/%s");
                assert_eq!(opener.name(), "Example");
            }

            #[test]
            #[serial]
            fn query_preserves_configuration() {
                let context = TestContext::new();
                Config::set_time_format(TimeFormat::Seconds).unwrap();
                let before = context.contents();

                context
                    .run(ConfigCommand::TimeFormat { value: None })
                    .unwrap();

                assert_eq!(context.contents(), before);
            }

            #[test]
            #[serial]
            fn propagates_invalid_configuration() {
                let context = TestContext::new();
                std::fs::write(context.directory.path().join("tlog.toml"), "invalid = [").unwrap();

                let result = handle_config_command(
                    ConfigCommand::TimeFormat {
                        value: Some(TimeFormat::Seconds),
                    },
                    &context.database,
                    &ConfigMetadata::default(),
                );

                assert!(matches!(
                    result.unwrap_err().downcast_ref::<ConfigError>(),
                    Some(ConfigError::TomlDeserialize { .. })
                ));
            }
        }

        mod opener {
            use super::*;

            #[test]
            #[serial]
            fn creates_opener_and_preserves_time_format() {
                let context = TestContext::new();
                Config::set_time_format(TimeFormat::Seconds).unwrap();

                context
                    .run(ConfigCommand::Opener {
                        url: Some("https://example.com/%s".into()),
                        name: Some("Example".into()),
                    })
                    .unwrap();

                let config = Config::get().unwrap();
                assert_eq!(config.time_format(), TimeFormat::Seconds);
                let opener = config.opener().as_ref().unwrap();
                assert_eq!(opener.url_template(), "https://example.com/%s");
                assert_eq!(opener.name(), "Example");
            }

            #[test]
            #[serial]
            fn updates_provided_fields_and_keeps_omitted_fields() {
                let context = TestContext::new();
                for (url, name, expected_url, expected_name) in [
                    (
                        Some("https://new.example/%s"),
                        None,
                        "https://new.example/%s",
                        "Original",
                    ),
                    (None, Some("Updated"), "https://old.example/%s", "Updated"),
                    (
                        Some("https://new.example/%s"),
                        Some("Updated"),
                        "https://new.example/%s",
                        "Updated",
                    ),
                ] {
                    Config::set_opener(Some(Opener::new("https://old.example/%s", "Original")))
                        .unwrap();

                    context
                        .run(ConfigCommand::Opener {
                            url: url.map(str::to_owned),
                            name: name.map(str::to_owned),
                        })
                        .unwrap();

                    let config = Config::get().unwrap();
                    let opener = config.opener().as_ref().unwrap();
                    assert_eq!(opener.url_template(), expected_url);
                    assert_eq!(opener.name(), expected_name);
                }
            }

            #[test]
            #[serial]
            fn creating_opener_requires_both_fields() {
                let context = TestContext::new();
                Config::get().unwrap();
                let before = context.contents();
                for (url, name, missing_field) in [
                    (Some("https://example.com/%s"), None, "name"),
                    (None, Some("Example"), "url"),
                ] {
                    let result = context.run(ConfigCommand::Opener {
                        url: url.map(str::to_owned),
                        name: name.map(str::to_owned),
                    });

                    assert!(matches!(result.unwrap_err().downcast_ref::<ConfigError>(),
                        Some(ConfigError::RequiredFieldMissing(field)) if *field == missing_field
                    ));
                    assert_eq!(context.contents(), before);
                }
            }

            #[test]
            #[serial]
            fn query_preserves_configuration_with_or_without_opener() {
                let context = TestContext::new();
                for opener in [
                    None,
                    Some(Opener::new("https://example.com/%s", "Open issue")),
                ] {
                    Config::set_opener(opener).unwrap();
                    let before = context.contents();

                    context
                        .run(ConfigCommand::Opener {
                            url: None,
                            name: None,
                        })
                        .unwrap();

                    assert_eq!(context.contents(), before);
                }
            }
        }
    }
}
