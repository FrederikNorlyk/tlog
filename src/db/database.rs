use crate::db::event_repository::EventRepository;
use crate::db::manual_session_repository::ManualSessionRepository;
use crate::db::project_repository::ProjectRepository;
use crate::core::paths::Paths;
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

pub struct Database {
    connection: Connection,
}

impl Database {
    /// Opens the application database connection, creating the data directory if needed.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::MissingDataDirectory`] if the operating system data
    /// directory cannot be determined.
    ///
    /// Returns [`DatabaseError::Io`] if the database parent directory cannot be created.
    ///
    /// Returns [`DatabaseError::Sqlite`] if opening the `SQLite` database fails.
    pub fn new() -> Result<Self, DatabaseError> {
        let db_path = Self::database_path()?;

        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        Ok(Self {
            connection: Connection::open(db_path)?,
        })
    }

    /// Opens the application database connection, creating the data directory if needed.
    ///
    /// # Errors
    ///
    /// Returns error if opening the database connection fails.
    pub fn new_in_memory_db() -> rusqlite::Result<Self> {
        Ok(Self {
            connection: Connection::open_in_memory()?,
        })
    }

    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    fn database_path() -> Result<PathBuf, DatabaseError> {
        let project_dirs = Paths::project_dir().ok_or(DatabaseError::MissingDataDirectory)?;

        Ok(project_dirs.data_dir().join("tlog.sqlite3"))
    }

    /// Initializes the database schema required by the application.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::Sqlite`] if creating or updating any repository schema
    /// fails.
    pub fn init(&self) -> Result<(), DatabaseError> {
        ProjectRepository::initialize_schema(self.connection())?;
        EventRepository::initialize_schema(self.connection())?;
        ManualSessionRepository::initialize_schema(self.connection())?;
        Ok(())
    }
}

pub trait Repository<'a> {
    /// Creates or updates the database schema required by the repository.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying database fails to execute the schema
    /// initialization statements.
    fn initialize_schema(connection: &'a Connection) -> rusqlite::Result<()>;
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Could not determine application data directory")]
    MissingDataDirectory,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}
