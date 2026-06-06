use crate::db::database::Database;
use crate::model::event::Event;
use crate::model::manual_session::ManualSession;
use rusqlite::Connection;
use time::Date;
use time::format_description::well_known::Iso8601;

pub struct DBTestContext {
    database: Database,
}

impl DBTestContext {
    /// Creates a new in-memory test database and initializes the schema.
    ///
    /// Intended for integration tests only.
    ///
    /// # Panics
    ///
    /// Panics if database schema initialization fails.
    ///
    /// # Errors
    ///
    /// Returns errors if database can not be initialized
    pub fn new() -> rusqlite::Result<Self> {
        let database = Database::new_in_memory_db()?;
        database.init().expect("Failed to initialize database");

        Ok(Self { database })
    }

    /// Returns a reference to the underlying `SQLite` connection.
    ///
    /// Used for executing ad-hoc test queries or repository construction.
    pub fn connection(&self) -> &Connection {
        self.database.connection()
    }

    /// Collects all manual sessions from the database.
    ///
    /// Converts raw database rows into [`ManualSession`] values for assertions.
    ///
    /// # Errors
    /// Returns a database error if query execution or row mapping fails.
    ///
    /// # Panics
    /// Panics if a stored date cannot be parsed (indicates corrupted test data).
    pub fn collect_sessions(&self) -> rusqlite::Result<Vec<ManualSession>> {
        let mut statement = self
            .database
            .connection()
            .prepare("SELECT * FROM manual_session")?;

        let rows = statement.query_map([], |row| {
            let string_date: String = row.get("date")?;

            let date =
                Date::parse(string_date.as_str(), &Iso8601::DATE).expect("Could not parse date");

            Ok(ManualSession {
                project_id: row.get("project_id")?,
                date,
                total_seconds: row.get("total_seconds")?,
            })
        })?;

        rows.collect()
    }

    /// Collects all events from the database.
    ///
    /// Used in tests to verify event persistence and ordering.
    ///
    /// # Errors
    /// Returns a database error if query execution or row mapping fails.
    pub fn collect_events(&self) -> rusqlite::Result<Vec<Event>> {
        let mut statement = self.database.connection().prepare("SELECT * FROM event")?;
        let rows = statement.query_map([], Event::from_row)?;

        rows.collect()
    }
}
