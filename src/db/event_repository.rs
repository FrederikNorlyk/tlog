use crate::db::database::Repository;
use crate::model::event::{Event, EventType};
use crate::model::session::Session;
use rusqlite::{Connection, OptionalExtension, Result, Row, named_params};
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
                Self::event_from_row,
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

    /// Calls the provided function once for each event in the database.
    ///
    /// # Errors
    ///
    /// Returns an error if preparing or executing the query fails, or if a row
    /// cannot be converted into an [`Event`].
    pub fn for_each<F>(&self, mut f: F) -> Result<()>
    where
        F: FnMut(Event) -> Result<()>,
    {
        let mut stmt = self.connection.prepare("SELECT * FROM event")?;

        let rows = stmt.query_map([], Self::event_from_row)?;

        for event in rows {
            f(event?)?;
        }

        Ok(())
    }

    /// Calls the provided function once for each latest started event per project.
    ///
    /// This selects projects whose most recent event is a start event, which can be
    /// used to find projects that currently have an active session.
    ///
    /// # Errors
    ///
    /// Returns an error if preparing or executing the query fails, if a returned row
    /// cannot be converted into an [`Event`], or if the provided callback returns an
    /// error.
    pub fn for_each_started_event<F>(&self, mut callback: F) -> Result<()>
    where
        F: FnMut(Event) -> Result<()>,
    {
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
            WHERE e.event_type = :start_event_type",
        )?;

        let rows = statement.query_map(
            named_params! {":start_event_type": EventType::START_CODE},
            Self::event_from_row,
        )?;

        for event in rows {
            callback(event?)?;
        }

        Ok(())
    }

    fn event_from_row(row: &Row<'_>) -> Result<Event> {
        Ok(Event {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            event_type: row.get("event_type")?,
            timestamp: row.get("timestamp")?,
        })
    }

    /// Calls the provided function once for each project duration matching the optional filters.
    ///
    /// If `date` is provided, it must be in `YYYY-MM-DD` format.
    ///
    /// The date filter counts only the part of each session that overlaps that day.
    /// For example, a session from `2026-01-25 23:30` to `2026-01-26 01:00`
    /// contributes one hour to `2026-01-26`.
    ///
    /// # Errors
    ///
    /// Returns an error if preparing or executing the query fails, or if a returned
    /// row cannot be converted into a [`ProjectDuration`].
    pub fn for_each_session<F>(
        &self,
        project_id: Option<i32>,
        date: Option<&str>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(Session) -> Result<()>,
    {
        let mut statement = self.connection.prepare(
            "WITH bounds AS (
                SELECT
                    CASE
                        WHEN :date IS NULL THEN NULL
                        ELSE unixepoch(:date)
                    END AS day_start,
                    CASE
                        WHEN :date IS NULL THEN NULL
                        ELSE unixepoch(:date, '+1 day')
                    END AS day_end
            ),
            sessions AS (
                SELECT
                    start_event.project_id,
                    start_event.timestamp AS start_timestamp,
                    (
                        SELECT MIN(stop_event.timestamp)
                        FROM event stop_event
                        WHERE stop_event.project_id = start_event.project_id
                          AND stop_event.event_type = :stop_event_type
                          AND stop_event.timestamp > start_event.timestamp
                    ) AS stop_timestamp
                FROM event start_event
                WHERE start_event.event_type = :start_event_type
            ),
            bounded_sessions AS (
                SELECT
                    sessions.project_id,
                    CASE
                        WHEN bounds.day_start IS NULL THEN sessions.start_timestamp
                        ELSE MAX(sessions.start_timestamp, bounds.day_start)
                    END AS bounded_start,
                    CASE
                        WHEN bounds.day_end IS NULL THEN sessions.stop_timestamp
                        ELSE MIN(sessions.stop_timestamp, bounds.day_end)
                    END AS bounded_stop
                FROM sessions
                CROSS JOIN bounds
                WHERE sessions.stop_timestamp IS NOT NULL
                  AND (:project_id IS NULL OR sessions.project_id = :project_id)
                  AND (
                      bounds.day_start IS NULL
                      OR (
                          sessions.start_timestamp < bounds.day_end
                          AND sessions.stop_timestamp > bounds.day_start
                      )
                  )
            )
            SELECT
                project_id,
                COALESCE(SUM(bounded_stop - bounded_start), 0) AS total_seconds
            FROM bounded_sessions
            GROUP BY project_id
            ORDER BY project_id",
        )?;

        let rows = statement.query_map(
            named_params! {
                ":project_id": project_id,
                ":date": date,
                ":start_event_type": EventType::START_CODE,
                ":stop_event_type": EventType::STOP_CODE,
            },
            |row| {
                Ok(Session {
                    project_id: row.get("project_id")?,
                    total_seconds: row.get("total_seconds")?,
                })
            },
        )?;

        for duration in rows {
            callback(duration?)?;
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
    use crate::db::database::Database;
    use crate::db::project_repository::ProjectRepository;
    use crate::model::event::EventType::{Start, Stop};
    use time::{Month, PrimitiveDateTime, Time};

    struct TestContext {
        database: Database,
    }

    impl TestContext {
        fn new() -> Result<Self> {
            let database = Database::new_in_memory_db()?;
            database.init().expect("Failed to initialize database");

            let project_repository = ProjectRepository::new(database.connection());
            project_repository.insert("Test name", None)?;
            project_repository.insert("Another project", None)?;
            project_repository.insert("Third project", None)?;

            Ok(Self { database })
        }

        fn event_repository(&self) -> EventRepository<'_> {
            EventRepository::new(self.database.connection())
        }
    }

    fn collect_sessions(
        repository: &EventRepository<'_>,
        project_id: Option<i32>,
        date: Option<&str>,
    ) -> Result<Vec<Session>> {
        let mut sessions = Vec::new();

        repository.for_each_session(project_id, date, |session| {
            sessions.push(session);
            Ok(())
        })?;

        Ok(sessions)
    }

    fn collect_events(repository: &EventRepository) -> Result<Vec<Event>> {
        let mut events = Vec::new();

        repository.for_each(|event| {
            events.push(event);
            Ok(())
        })?;

        Ok(events)
    }

    fn collect_started_events(repository: &EventRepository<'_>) -> Result<Vec<Event>> {
        let mut events = Vec::new();

        repository.for_each_started_event(|event| {
            events.push(event);
            Ok(())
        })?;

        Ok(events)
    }

    fn assert_single_session(
        sessions: &[Session],
        expected_project_id: i32,
        expected_total_seconds: i64,
    ) {
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].project_id, expected_project_id);
        assert_eq!(sessions[0].total_seconds, expected_total_seconds);
    }

    #[test]
    fn test_insert_and_get_event() -> Result<()> {
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

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
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

        let event = event_repository.get(999)?;

        assert!(event.is_none());

        Ok(())
    }

    #[test]
    fn test_delete_event() -> Result<()> {
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

        event_repository.insert(1, Start, 1_780_140_094)?;

        assert!(event_repository.get(1)?.is_some());

        let deleted = event_repository.delete(1)?;

        assert!(deleted);
        assert!(event_repository.get(1)?.is_none());

        Ok(())
    }

    #[test]
    fn test_delete_missing_event_returns_false() -> Result<()> {
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

        let deleted = event_repository.delete(999)?;

        assert!(!deleted);

        Ok(())
    }

    #[test]
    fn test_delete_all_in() -> Result<(), Box<dyn std::error::Error>> {
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

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

        let mut events = collect_events(&event_repository)?;

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

        events = collect_events(&event_repository)?;

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
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

        let mut timestamp = 1_780_140_094;

        event_repository.insert(1, Start, timestamp)?;
        timestamp += 300;
        event_repository.insert(1, Stop, timestamp)?;

        timestamp += 700;
        event_repository.insert(2, Start, timestamp)?;

        let started_events = collect_started_events(&event_repository)?;

        assert_eq!(started_events.len(), 1);
        assert_eq!(started_events[0].project_id, 2);
        assert!(matches!(started_events[0].event_type, Start));
        assert_eq!(started_events[0].timestamp, timestamp);

        Ok(())
    }

    #[test]
    fn test_for_each_started_event_uses_latest_event_per_project() -> Result<()> {
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

        let mut timestamp = 1_780_140_094;

        event_repository.insert(1, Start, timestamp)?;
        timestamp += 300;
        event_repository.insert(1, Stop, timestamp)?;

        timestamp += 700;
        event_repository.insert(1, Start, timestamp)?;
        let expected_project_1_start_timestamp = timestamp;

        timestamp += 300;
        event_repository.insert(2, Start, timestamp)?;
        timestamp += 300;
        event_repository.insert(2, Stop, timestamp)?;

        timestamp += 400;
        event_repository.insert(3, Start, timestamp)?;
        let expected_project_3_start_timestamp = timestamp;

        let started_events = collect_started_events(&event_repository)?;

        let project_1_started_event = started_events
            .iter()
            .find(|event| event.project_id == 1)
            .expect("project 1 started event should exist");

        let project_3_started_event = started_events
            .iter()
            .find(|event| event.project_id == 3)
            .expect("project 3 started event should exist");

        assert_eq!(started_events.len(), 2);

        assert!(matches!(project_1_started_event.event_type, Start));
        assert_eq!(
            project_1_started_event.timestamp,
            expected_project_1_start_timestamp
        );

        assert!(matches!(project_3_started_event.event_type, Start));
        assert_eq!(
            project_3_started_event.timestamp,
            expected_project_3_start_timestamp
        );

        Ok(())
    }

    #[test]
    fn test_session_duration_can_be_filtered_by_date() -> Result<()> {
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

        let mut timestamp = 1_767_308_400;

        event_repository.insert(1, Start, timestamp)?;
        timestamp += 7200;
        event_repository.insert(1, Stop, timestamp)?;

        let sessions = collect_sessions(&event_repository, Some(1), Some("2026-01-01"))?;
        assert_single_session(&sessions, 1, 3600);

        let sessions = collect_sessions(&event_repository, Some(1), Some("2026-01-02"))?;
        assert_single_session(&sessions, 1, 3600);

        let sessions = collect_sessions(&event_repository, Some(1), Some("2026-01-03"))?;
        assert!(sessions.is_empty());

        Ok(())
    }

    #[test]
    fn test_session_duration_calculation() -> Result<()> {
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

        let mut timestamp = 1_780_140_094;

        event_repository.insert(1, Start, timestamp)?;
        timestamp += 300;
        event_repository.insert(1, Stop, timestamp)?;

        let sessions = collect_sessions(&event_repository, Some(1), None)?;
        assert_single_session(&sessions, 1, 300);

        // Calculation should not include time between sessions
        timestamp += 4000;
        event_repository.insert(1, Start, timestamp)?;

        timestamp += 700;
        event_repository.insert(1, Stop, timestamp)?;

        let sessions = collect_sessions(&event_repository, Some(1), None)?;
        assert_single_session(&sessions, 1, 1000);

        event_repository.insert(2, Start, timestamp)?;
        timestamp += 800;
        event_repository.insert(2, Stop, timestamp)?;

        let sessions = collect_sessions(&event_repository, Some(2), None)?;
        assert_single_session(&sessions, 2, 800);

        let sessions = collect_sessions(&event_repository, None, None)?;

        let project_1_session = sessions
            .iter()
            .find(|session| session.project_id == 1)
            .expect("project 1 session should exist");

        let project_2_session = sessions
            .iter()
            .find(|session| session.project_id == 2)
            .expect("project 2 session should exist");

        assert_eq!(project_1_session.total_seconds, 1000);
        assert_eq!(project_2_session.total_seconds, 800);
        assert_eq!(sessions.len(), 2);

        Ok(())
    }

    #[test]
    fn test_event_cannot_be_created_for_missing_project() -> Result<()> {
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

        let result = event_repository.insert(999, Start, 1_780_140_094);

        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_deleting_project_deletes_its_events() -> Result<()> {
        let context = TestContext::new()?;
        let project_repository = ProjectRepository::new(context.database.connection());
        let event_repository = context.event_repository();

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

    #[test]
    fn test_ongoing_session_not_included() -> Result<()> {
        let context = TestContext::new()?;
        let event_repository = context.event_repository();

        let mut timestamp = 1_780_140_094;
        event_repository.insert(1, Start, timestamp)?;

        let sessions = collect_sessions(&event_repository, Some(1), None)?;
        assert!(sessions.is_empty());

        timestamp += 500;
        event_repository.insert(1, Stop, timestamp)?;

        timestamp += 5000;
        event_repository.insert(1, Start, timestamp)?;

        let sessions = collect_sessions(&event_repository, Some(1), None)?;
        assert_single_session(&sessions, 1, 500);

        Ok(())
    }
}
