use crate::core::unix_timestamp::UnixTimestamp;
use crate::db::event_repository::EventRepository;
use crate::model::event::EventType;
use rusqlite::Connection;
use std::error::Error;
use std::fmt::{Display, Formatter};

pub struct Tracking<'a> {
    connection: &'a Connection,
}

impl<'a> Tracking<'a> {
    #[must_use]
    pub fn new(connection: &'a Connection) -> Self {
        Self { connection }
    }

    pub fn start(&self, project_id: i32) -> Result<(), TrackingError> {
        let event_repository = EventRepository::new(self.connection);
        let timestamp = UnixTimestamp::now();

        event_repository.for_each_started_event(|event| {
            event_repository.insert(event.project_id, EventType::Stop, timestamp)?;
            Ok(())
        })?;

        event_repository.insert(project_id, EventType::Start, timestamp)?;

        Ok(())
    }

    pub fn stop(&self, project_id: i32) -> Result<(), TrackingError> {
        let event_repository = EventRepository::new(self.connection);

        let timestamp = UnixTimestamp::now();

        event_repository.insert(project_id, EventType::Stop, timestamp)?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum TrackingError {
    Sqlite(rusqlite::Error),
}

impl Error for TrackingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
        }
    }
}

impl Display for TrackingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "SQLite error: {error}"),
        }
    }
}

impl From<rusqlite::Error> for TrackingError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}
