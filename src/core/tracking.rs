use crate::core::unix_timestamp::UnixTimestamp;
use crate::db::event_repository::EventRepository;
use crate::model::event::EventType;
use rusqlite::Connection;
use thiserror::Error;
use time::{Date, PrimitiveDateTime, Time};

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
    pub fn stop(&self, project_id: i32) -> Result<(), TrackingError> {
        let event_repository = EventRepository::new(self.connection);
        let timestamp = UnixTimestamp::now();

        if !event_repository.has_started_event(project_id)? {
            return Err(TrackingError::NoActiveStartEvent { project_id });
        }

        // TODO: Clamp within same date (if we have progressed to the next date, create new start stop events in that one.

        event_repository.insert(project_id, EventType::Stop, timestamp)?;

        Ok(())
    }

    /// Explicitly sets the time spent on a given project on a given date.
    ///
    /// It does so by removing any start / stop events on the given project on the given date,
    /// and instead creates a manual duration.
    ///
    /// # Errors
    ///
    /// Returns an error if deleting or creating events in the database fails.
    pub fn manual_session(
        &self,
        project_id: i32,
        date: Date,
        duration: i64,
    ) -> rusqlite::Result<()> {
        let event_repository = EventRepository::new(self.connection);

        event_repository.delete_all_in(project_id, date)?;

        let start = PrimitiveDateTime::new(date, Time::MIDNIGHT)
            .assume_utc()
            .unix_timestamp();

        let end = start + duration;

        // TODO: Don't create events, instead create a new table for manual sessions / paddings / etc.
        event_repository.insert(project_id, EventType::Start, start)?;
        event_repository.insert(project_id, EventType::Stop, end)?;

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum TrackingError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("no active start event for project {project_id}")]
    NoActiveStartEvent { project_id: i32 },
}

// TODO: Test coverage with cargo-llvm-cov (add to devcontainer. Make CI fail on too low coverage)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::database::Database;
    use crate::db::event_repository::EventRepository;
    use crate::db::project_repository::ProjectRepository;
    use crate::model::event::EventType::{Start, Stop};
    use crate::model::event::{Event, EventType};
    use time::Month;

    struct TestContext {
        database: Database,
    }

    impl TestContext {
        fn new() -> rusqlite::Result<Self> {
            let database = Database::new_in_memory_db()?;
            database.init().expect("Failed to initialize database");

            let project_repository = ProjectRepository::new(database.connection());
            project_repository.insert("First", None)?;
            project_repository.insert("Second", None)?;
            project_repository.insert("Third", None)?;

            Ok(Self { database })
        }

        fn all_events(&self) -> rusqlite::Result<Vec<Event>> {
            let event_repository = EventRepository::new(self.database.connection());

            let mut events = Vec::new();

            event_repository.for_each(|event| {
                events.push(event);
                Ok(())
            })?;

            Ok(events)
        }
    }

    #[test]
    fn test_start_and_stop() -> Result<(), TrackingError> {
        let context = TestContext::new()?;

        let tracking = Tracking::new(context.database.connection());

        tracking.start(1)?;
        tracking.stop(1)?;

        let events = context.all_events()?;

        assert_eq!(events.len(), 2);

        let start_event = &events[0];
        assert_eq!(start_event.project_id, 1);
        assert_eq!(start_event.event_type, Start);

        let stop_event = &events[1];
        assert_eq!(stop_event.project_id, 1);
        assert_eq!(stop_event.event_type, Stop);

        Ok(())
    }

    #[test]
    fn start_stops_existing_started_events_before_starting_new_project() -> rusqlite::Result<()> {
        let context = TestContext::new()?;
        let tracking = Tracking::new(context.database.connection());

        tracking.start(1)?;
        tracking.start(2)?;

        let events = context.all_events()?;

        assert_eq!(events.len(), 3);

        assert_eq!(events[0].project_id, 1);
        assert!(matches!(events[0].event_type, EventType::Start));

        assert_eq!(events[1].project_id, 1);
        assert!(matches!(events[1].event_type, EventType::Stop));

        assert_eq!(events[2].project_id, 2);
        assert!(matches!(events[2].event_type, EventType::Start));

        Ok(())
    }

    #[test]
    fn stop_without_start_will_fail() -> Result<(), TrackingError> {
        let context = TestContext::new()?;
        let tracking = Tracking::new(context.database.connection());
        let result = tracking.stop(1);

        assert!(matches!(
            result,
            Err(TrackingError::NoActiveStartEvent { project_id }) if project_id == 1
        ));

        Ok(())
    }

    #[test]
    fn test_manual_session() -> Result<(), Box<dyn std::error::Error>> {
        let context = TestContext::new()?;

        create_and_assert_manual_session(&context)?;

        Ok(())
    }

    #[test]
    fn test_manual_session_deletes_previous_events() -> Result<(), Box<dyn std::error::Error>> {
        let context = TestContext::new()?;
        let event_repository = EventRepository::new(context.database.connection());
        let start_date = Date::from_calendar_date(2026, Month::June, 15)?;
        let start_time = Time::from_hms(3, 25, 55)?;

        let mut timestamp = PrimitiveDateTime::new(start_date, start_time)
            .assume_utc()
            .unix_timestamp();

        event_repository.insert(1, Start, timestamp)?;

        timestamp += 700;

        event_repository.insert(1, Stop, timestamp)?;

        timestamp += 6000;

        event_repository.insert(1, Start, timestamp)?;

        timestamp += 2000;

        event_repository.insert(1, Stop, timestamp)?;

        assert_eq!(context.all_events()?.len(), 4);

        create_and_assert_manual_session(&context)?;

        Ok(())
    }

    fn create_and_assert_manual_session(context: &TestContext) -> rusqlite::Result<()> {
        let tracking = Tracking::new(context.database.connection());

        let start_date =
            Date::from_calendar_date(2026, Month::June, 15).expect("Could not initialize date");

        tracking.manual_session(1, start_date, 600)?;

        let events = context.all_events()?;
        assert_eq!(events.len(), 2);

        let start_event = events.first().expect("Could not find event");
        assert_eq!(start_event.project_id, 1);
        assert_eq!(start_event.event_type, Start);
        assert_eq!(start_event.timestamp, 1_781_481_600);

        let end_event = events.get(1).expect("Could not find event");
        assert_eq!(end_event.project_id, 1);
        assert_eq!(end_event.event_type, Stop);
        assert_eq!(end_event.timestamp, 1_781_482_200);

        Ok(())
    }
}
