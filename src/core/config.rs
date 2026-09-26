use crate::core::issue_tracker::IssueTracker;
use crate::core::paths::Paths;
use crate::core::time_format::TimeFormat;
use crate::model::opener::Opener;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

pub struct Config;

impl Config {
    /// Get the app's configuration.
    ///
    /// # Errors
    /// Returns an error if reading or writing to files failed.
    pub fn get() -> Result<ConfigMetadata, ConfigError> {
        let path = Config::get_or_create_file_path()?;
        let contents = fs::read_to_string(&path).map_err(|source| ConfigError::Io {
            operation: "read configuration",
            path: path.clone(),
            source,
        })?;
        toml::from_str(&contents).map_err(|source| ConfigError::TomlDeserialize { path, source })
    }

    /// Sets the app's time format
    ///
    /// # Errors
    /// Returns an error if reading or writing to files failed.
    pub fn set_time_format(time_format: TimeFormat) -> Result<(), ConfigError> {
        let mut config = Config::get()?;
        config.time_format = time_format;

        Self::write(&config)?;

        Ok(())
    }

    /// Sets the app's opener
    ///
    /// # Errors
    /// Returns an error if reading or writing to files failed.
    pub fn set_opener(opener: Option<Opener>) -> Result<(), ConfigError> {
        let mut config = Config::get()?;
        config.opener = opener;

        Self::write(&config)?;

        Ok(())
    }

    /// Returns the path to the app's configuration file.
    /// If the file doesn't exist, it attempts to create it.
    ///
    /// # Errors
    /// Returns an error if reading or writing to files failed.
    pub fn get_or_create_file_path() -> Result<PathBuf, ConfigError> {
        let path = Self::file_path()?;

        if !path.exists() {
            Self::write(&ConfigMetadata::default())?;
        }

        Ok(path)
    }

    fn write(config: &ConfigMetadata) -> Result<(), ConfigError> {
        let path = Self::file_path()?;
        let toml_str =
            toml::to_string_pretty(config).map_err(|source| ConfigError::TomlSerialize {
                path: path.clone(),
                source,
            })?;

        fs::write(&path, toml_str).map_err(|source| ConfigError::Io {
            operation: "write configuration",
            path,
            source,
        })?;

        Ok(())
    }

    fn file_path() -> Result<PathBuf, ConfigError> {
        let config_dir = Paths::config_dir().ok_or(ConfigError::MissingConfigDirectory)?;
        let path = config_dir.join("tlog.toml");

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
                operation: "create configuration directory",
                path: parent.to_path_buf(),
                source,
            })?;
        }

        Ok(path)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ConfigMetadata {
    time_format: TimeFormat,
    opener: Option<Opener>,
    issue_tracker: Option<IssueTracker>,
}

impl Default for ConfigMetadata {
    fn default() -> Self {
        Self {
            time_format: TimeFormat::HoursMinutesSeconds,
            opener: None,
            issue_tracker: None,
        }
    }
}

impl ConfigMetadata {
    #[must_use]
    pub fn time_format(&self) -> TimeFormat {
        self.time_format
    }

    #[must_use]
    pub fn opener(&self) -> &Option<Opener> {
        &self.opener
    }

    #[must_use]
    pub fn issue_tracker(&self) -> &Option<IssueTracker> {
        &self.issue_tracker
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Could not determine application data config")]
    MissingConfigDirectory,
    #[error("Could not determine application data directory")]
    MissingDataDirectory,
    #[error("Could not {operation} at {}", path.display())]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Could not parse configuration {}", path.display())]
    TomlDeserialize {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("Could not serialize configuration {}", path.display())]
    TomlSerialize {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
    #[error("{0} is required")]
    RequiredFieldMissing(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::constants::CONFIG_DIR_ENV;
    use serial_test::serial;

    #[allow(unsafe_code)]
    fn init_new_temp_dir() {
        let temp = tempfile::tempdir().unwrap();
        println!("Running tests in {}", temp.path().display());

        unsafe {
            std::env::set_var(CONFIG_DIR_ENV, temp.path().join("config"));
        }
    }

    #[allow(unsafe_code)]
    fn teardown() {
        unsafe {
            std::env::remove_var(CONFIG_DIR_ENV);
        }
    }

    #[test]
    #[serial]
    fn set_time_format() {
        init_new_temp_dir();

        Config::set_time_format(TimeFormat::DecimalHours).unwrap();

        let config = Config::get().unwrap();

        assert_eq!(TimeFormat::DecimalHours, config.time_format);
        assert!(config.opener.is_none());

        teardown();
    }

    mod get {
        use super::*;
        use serial_test::serial;

        #[test]
        #[serial]
        fn default_values() {
            init_new_temp_dir();

            let config = Config::get().unwrap();

            assert_eq!(TimeFormat::HoursMinutesSeconds, config.time_format);
            assert!(config.opener.is_none());

            teardown();
        }

        #[test]
        #[serial]
        fn overwritten_values() {
            init_new_temp_dir();

            let mut config = Config::get().unwrap();
            config.time_format = TimeFormat::Seconds;

            config.opener = Some(Opener::new("https://www.test.site/search/%s", "Test"));

            Config::write(&config).unwrap();

            let config = Config::get().unwrap();
            let opener = config.opener.unwrap();

            assert_eq!(TimeFormat::Seconds, config.time_format);
            assert_eq!("https://www.test.site/search/%s", opener.url_template());
            assert_eq!("Test", opener.name());

            teardown();
        }
    }
}
