use crate::core::paths::Paths;
use crate::core::time_format::TimeFormat;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

pub struct Config;

impl Config {
    pub fn get() -> Result<ConfigMetadata, ConfigError> {
        let path = Config::get_or_create_file_path()?;
        let contents = fs::read_to_string(path)?;
        Ok(toml::from_str(&contents)?)
    }

    pub fn set_time_format(time_format: TimeFormat) -> Result<(), ConfigError> {
        let mut config = Config::get()?;
        config.time_format = time_format;

        Self::write(config)?;

        Ok(())
    }

    pub fn get_or_create_file_path() -> Result<PathBuf, ConfigError> {
        let path = Self::file_path()?;

        if !path.exists() {
            Self::write(ConfigMetadata::default())?;
        }

        Ok(path)
    }

    fn write(config: ConfigMetadata) -> Result<(), ConfigError> {
        let path = Self::file_path()?;
        let toml_str = toml::to_string_pretty(&config)?;

        fs::write(&path, toml_str)?;

        Ok(())
    }

    fn file_path() -> Result<PathBuf, ConfigError> {
        let project_dirs = Paths::project_dir().ok_or(ConfigError::MissingDataDirectory)?;
        let dir = project_dirs.config_dir();
        let path = dir.join("tlog.toml");

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        Ok(path)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ConfigMetadata {
    time_format: TimeFormat,
}

impl Default for ConfigMetadata {
    fn default() -> Self {
        Self {
            time_format: TimeFormat::HoursMinutesSeconds,
        }
    }
}

impl ConfigMetadata {
    pub fn time_format(&self) -> TimeFormat {
        self.time_format
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Could not determine application data directory")]
    MissingDataDirectory,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Toml deserialization error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("Toml serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}
