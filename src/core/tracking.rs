use crate::core::unix_timestamp::UnixTimestamp;
use crate::db::event_repository::EventRepository;
use crate::model::event::EventType;
use rusqlite::Connection;

pub struct Tracking<'a> {
    connection: &'a Connection,
}

impl<'a> Tracking<'a> {
    #[must_use]
    pub fn new(connection: &'a Connection) -> Self {
        Self { connection }
    }

    /// Starts tracking time for the given project.
    ///
    /// Any currently started events are stopped before the new start event is inserted.
    ///
    /// # Errors
    ///
    /// Returns an error if reading existing started events or inserting
    /// the generated stop/start events into the database fails.
    pub fn start(&self, project_id: i32) -> rusqlite::Result<()> {
        let event_repository = EventRepository::new(self.connection);
        let timestamp = UnixTimestamp::now();

        event_repository.for_each_started_event(|event| {
            event_repository.insert(event.project_id, EventType::Stop, timestamp)?;
            Ok(())
        })?;

        event_repository.insert(project_id, EventType::Start, timestamp)?;

        Ok(())
    }

    /// Stops tracking time for the given project.
    ///
    /// # Errors
    ///
    /// Returns an error if inserting the stop event into the database fails.
    pub fn stop(&self, project_id: i32) -> rusqlite::Result<()> {
        let event_repository = EventRepository::new(self.connection);

        let timestamp = UnixTimestamp::now();

        event_repository.insert(project_id, EventType::Stop, timestamp)?;

        Ok(())
    }
}
