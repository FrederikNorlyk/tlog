use crate::db::database::Repository;
use crate::model::event::{Event, EventType};
use rusqlite::{named_params, Connection, OptionalExtension, Result};
use time::Date;

pub struct EventRepository<'a> {
    connection: &'a Connection,
}

impl<'a> EventRepository<'a> {
    #[must_use]
    pub fn new(connection: &'a Connection) -> Self {
        Self { connection }
    }

    /// Inserts a new event.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the insert statement, for
    /// example because the database connection is invalid, the `event` table
    /// does not exist, or the provided data violates a database constraint.
    pub fn insert(&self, project_id: i32, event_type: EventType, timestamp: i64) -> Result<()> {
        self.connection.execute(
            "INSERT INTO event (project_id, event_type, timestamp)
            VALUES (:project_id, :event_type, :timestamp)",
            named_params! {":project_id": project_id, ":event_type": event_type, ":timestamp": timestamp},
        )?;

        Ok(())
    }

    /// Deletes the event with the given ID.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the delete statement, for
    /// example because the database connection is invalid or the `event` table
    /// does not exist.
    pub fn delete(&self, id: i32) -> Result<bool> {
        let deleted_count = self.connection.execute(
            "DELETE FROM event WHERE id = (:id)",
            named_params! {":id": id},
        )?;

        Ok(deleted_count > 0)
    }

    /// Deletes all events for the given project on the given date.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the delete statement, for
    /// example because the database connection is invalid or the `event` table
    /// does not exist.
    pub fn delete_all_in(&self, project_id: i32, date: Date) -> Result<bool> {
        let deleted_count = self.connection.execute(
            "DELETE FROM event
            WHERE
                project_id = :project_id AND
                timestamp >= unixepoch(:date) AND
                timestamp < unixepoch(:date, '+1 day')",
            named_params! {":project_id": project_id, ":date": date.to_string()},
        )?;

        Ok(deleted_count > 0)
    }

    /// Gets the event with the given ID.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the query, if the `event`
    /// table does not exist, if no event exists with the given ID, or if the
    /// returned row cannot be converted into an [`Event`].
    pub fn get(&self, id: i32) -> Result<Option<Event>> {
        self.connection
            .query_row(
                "SELECT id, project_id, event_type, timestamp FROM event WHERE id = :id",
                named_params! {":id": id},
                Event::from_row,
            )
            .optional()
    }

    /// Returns whether the given project currently has an active start event.
    ///
    /// A project is considered active if its latest event is a `Start` event.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying `SQLite` query fails.
    pub fn has_started_event(&self, project_id: i32) -> Result<bool> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(
            SELECT 1
            FROM event e
            INNER JOIN (
                SELECT project_id, MAX(timestamp) AS max_timestamp
                FROM event
                GROUP BY project_id
            ) latest
                ON e.project_id = latest.project_id
                AND e.timestamp = latest.max_timestamp
            WHERE e.project_id = :project_id
              AND e.event_type = :start_event_type
        )",
            named_params! {
                ":project_id": project_id,
                ":start_event_type": EventType::START_CODE,
            },
            |row| row.get(0),
        )?;

        Ok(exists)
    }

    /// Calls the provided function once for each latest started event per project.
    ///
    /// This selects projects whose most recent event is a start event, which can be
    /// used to find projects that currently have an active session.
    ///
    /// # Errors
    ///
    /// Returns an error if preparing or executing the query fails, if a returned row
    /// cannot be converted into an [`Event`], or if the provided consumer returns an
    /// error.
    pub fn get_started_event(&self) -> Result<Option<Event>> {
        let mut statement = self.connection.prepare(
            "SELECT e.id, e.project_id, e.event_type, e.timestamp
            FROM event e
            INNER JOIN (
                SELECT project_id, MAX(timestamp) AS max_timestamp
                FROM event
                GROUP BY project_id
            ) latest
                ON e.project_id = latest.project_id
                AND e.timestamp = latest.max_timestamp
            WHERE e.event_type = :start_event_type
            LIMIT 1",
        )?;

        let mut rows =
            statement.query(named_params! {":start_event_type": EventType::START_CODE})?;

        match rows.next()? {
            Some(row) => Ok(Some(Event::from_row(row)?)),
            None => Ok(None),
        }
    }

    /// Iterates over all events for a given date in chronological order.
    ///
    /// Events are streamed (not collected) and passed one-by-one to the provided callback.
    ///
    /// # Behavior
    ///
    /// - Only events within the given calendar date are included
    /// - Results are ordered by timestamp ascending
    /// - Events are not buffered in memory beyond iteration
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or row mapping fails.
    pub fn for_each<F>(&self, date: Date, mut consumer: F) -> Result<()>
    where
        F: FnMut(Event),
    {
        let mut statement = self.connection.prepare(
            "SELECT * FROM event
            WHERE timestamp >= unixepoch(:date) AND timestamp < unixepoch(:date, '+1 day')
            ORDER BY timestamp",
        )?;

        let rows =
            statement.query_map(named_params! {":date": date.to_string()}, Event::from_row)?;

        for event in rows {
            consumer(event?);
        }

        Ok(())
    }
}

impl<'a> Repository<'a> for EventRepository<'a> {
    fn initialize_schema(connection: &'a Connection) -> Result<()> {
        connection.execute(
            "CREATE TABLE IF NOT EXISTS event (
                id INTEGER PRIMARY KEY,
                project_id INTEGER NOT NULL,
                event_type INTEGER NOT NULL CHECK(event_type IN (0, 1)),
                timestamp INTEGER NOT NULL,
                FOREIGN KEY(project_id)
                    REFERENCES project(id)
                    ON DELETE CASCADE
            )",
            (),
        )?;

        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_event_project_timestamp
            ON event(project_id, timestamp DESC)",
            (),
        )?;

        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_event_project_type_timestamp
            ON event(project_id, event_type, timestamp)",
            (),
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::project_repository::ProjectRepository;
    use crate::db::test_utils::DBTestContext;
    use crate::model::event::EventType::{Start, Stop};
    use std::error::Error;
    use time::{Month, PrimitiveDateTime, Time};

    fn initialize_context() -> Result<DBTestContext> {
        let context = DBTestContext::new()?;
        let project_repository = ProjectRepository::new(context.connection());

        project_repository.insert("Test name", None)?;
        project_repository.insert("Another project", None)?;
        project_repository.insert("Third project", None)?;

        Ok(context)
    }

    #[test]
    fn test_insert_and_get_event() -> Result<()> {
        let context = initialize_context()?;
        let event_repository = EventRepository::new(context.connection());
        let timestamp = 1_780_140_094;

        event_repository.insert(1, Start, timestamp)?;

        let event = event_repository
            .get(1)?
            .expect("inserted event should exist");

        assert_eq!(event.id, 1);
        assert_eq!(event.project_id, 1);
        assert!(matches!(event.event_type, Start));
        assert_eq!(event.timestamp, timestamp);

        Ok(())
    }

    #[test]
    fn test_get_missing_event_returns_none() -> Result<()> {
        let context = initialize_context()?;
        let event_repository = EventRepository::new(context.connection());

        let event = event_repository.get(999)?;

        assert!(event.is_none());

        Ok(())
    }

    #[test]
    fn test_delete_event() -> Result<()> {
        let context = initialize_context()?;
        let event_repository = EventRepository::new(context.connection());

        event_repository.insert(1, Start, 1_780_140_094)?;

        assert!(event_repository.get(1)?.is_some());

        let deleted = event_repository.delete(1)?;

        assert!(deleted);
        assert!(event_repository.get(1)?.is_none());

        Ok(())
    }

    #[test]
    fn test_delete_missing_event_returns_false() -> Result<()> {
        let context = initialize_context()?;
        let event_repository = EventRepository::new(context.connection());

        let deleted = event_repository.delete(999)?;

        assert!(!deleted);

        Ok(())
    }

    #[test]
    fn test_delete_all_in() -> Result<(), Box<dyn Error>> {
        let context = initialize_context()?;
        let event_repository = EventRepository::new(context.connection());

        let start_date = Date::from_calendar_date(2024, Month::September, 20)?;
        let time = Time::from_hms(2, 30, 00)?;

        let mut timestamp = PrimitiveDateTime::new(start_date, time)
            .assume_utc()
            .unix_timestamp();

        event_repository.insert(1, Start, timestamp)?;
        timestamp += 500;
        event_repository.insert(1, Stop, timestamp)?;
        timestamp += 500;
        event_repository.insert(2, Start, timestamp)?;
        timestamp += 500;
        event_repository.insert(2, Stop, timestamp)?;

        let mut did_delete = event_repository.delete_all_in(1, start_date)?;

        assert!(did_delete);

        let events = context.collect_events()?;

        assert_eq!(events.len(), 2);

        for event in events {
            assert_eq!(event.project_id, 2);
        }

        let next_date = Date::from_calendar_date(2024, Month::September, 21)?;

        let mut next_date_timestamp = PrimitiveDateTime::new(next_date, time)
            .assume_utc()
            .unix_timestamp();

        // Event with id 5
        event_repository.insert(2, Start, next_date_timestamp)?;

        next_date_timestamp += 500;

        // Event with id 6
        event_repository.insert(2, Stop, next_date_timestamp)?;

        did_delete = event_repository.delete_all_in(2, start_date)?;

        assert!(did_delete);

        let events = context.collect_events()?;

        assert_eq!(events.len(), 2);

        let start_event = events.first().expect("Could not get event");
        assert_eq!(start_event.id, 5);
        assert_eq!(start_event.project_id, 2);
        assert_eq!(start_event.event_type, Start);
        assert_eq!(start_event.timestamp, 1_726_885_800);

        let end_event = events.get(1).expect("Could not get event");
        assert_eq!(end_event.id, 6);
        assert_eq!(end_event.project_id, 2);
        assert_eq!(end_event.event_type, Stop);
        assert_eq!(end_event.timestamp, 1_726_886_300);

        Ok(())
    }

    #[test]
    fn test_for_each_started_event_returns_projects_with_latest_start_event() -> Result<()> {
        let context = initialize_context()?;
        let event_repository = EventRepository::new(context.connection());

        let mut timestamp = 1_780_140_094;

        event_repository.insert(1, Start, timestamp)?;
        timestamp += 300;
        event_repository.insert(1, Stop, timestamp)?;

        timestamp += 700;
        event_repository.insert(2, Start, timestamp)?;

        let started_event = event_repository
            .get_started_event()?
            .expect("expected a started event");

        assert_eq!(started_event.project_id, 2);
        assert!(matches!(started_event.event_type, Start));
        assert_eq!(started_event.timestamp, timestamp);

        Ok(())
    }

    #[test]
    fn test_event_cannot_be_created_for_missing_project() -> Result<()> {
        let context = initialize_context()?;
        let event_repository = EventRepository::new(context.connection());

        let result = event_repository.insert(999, Start, 1_780_140_094);

        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_deleting_project_deletes_its_events() -> Result<()> {
        let context = initialize_context()?;
        let project_repository = ProjectRepository::new(context.connection());
        let event_repository = EventRepository::new(context.connection());

        let mut timestamp = 1_780_140_094;

        event_repository.insert(1, Start, timestamp)?;
        timestamp += 300;
        event_repository.insert(1, Stop, timestamp)?;

        timestamp += 700;
        event_repository.insert(2, Start, timestamp)?;

        assert!(event_repository.get(1)?.is_some());
        assert!(event_repository.get(2)?.is_some());
        assert!(event_repository.get(3)?.is_some());

        let deleted = project_repository.delete(1)?;

        assert!(deleted);
        assert!(event_repository.get(1)?.is_none());
        assert!(event_repository.get(2)?.is_none());
        assert!(event_repository.get(3)?.is_some());

        Ok(())
    }
}
