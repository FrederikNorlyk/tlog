use crate::core::config::ConfigError;
use crate::core::tracking::TrackingError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Tracking error: {0}")]
    Tracking(#[from] TrackingError),
    #[error("Clipboard error: {0}")]
    Clipboard(#[from] arboard::Error),
    #[error("Invalid state: {message}")]
    InvalidState { message: &'static str },
    #[error("Config error: {0}")]
    Config(#[from] ConfigError),
}
