use crate::db::event_repository::EventRepository;
use crate::db::project_repository::ProjectRepository;
use directories::ProjectDirs;
use rusqlite::Connection;
use std::error::Error;
use std::fmt::Formatter;
use std::path::PathBuf;
use std::{fmt, fs};

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
        let project_dirs = ProjectDirs::from("com", "FrederikNorlyk", "tlog")
            .ok_or(DatabaseError::MissingDataDirectory)?;

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

// TODO: Use derives from thiserror
#[derive(Debug)]
pub enum DatabaseError {
    MissingDataDirectory,
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
}

impl Error for DatabaseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingDataDirectory => None,
            Self::Io(error) => Some(error),
            Self::Sqlite(error) => Some(error),
        }
    }
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDataDirectory => {
                write!(f, "Could not determine application data directory")
            }
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Sqlite(error) => write!(f, "SQLite error: {error}"),
        }
    }
}

impl From<std::io::Error> for DatabaseError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}
