use crate::db::database::Repository;
use crate::model::event::{Event, EventType};
use crate::model::session::Session;
use rusqlite::{named_params, Connection, OptionalExtension, Result, Row};

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

    /// Updates an existing event.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the update statement, for
    /// example because the database connection is invalid, the `event` table
    /// does not exist, or the provided data violates a database constraint.
    pub fn update(&self, event: &Event) -> Result<()> {
        self.connection.execute(
            "UPDATE event
            SET project_id = :project_id, event_type = :event_type, timestamp = :timestamp
            WHERE id = :id",
            named_params! {
                ":project_id": event.project_id,
                ":event_type": event.event_type,
                ":timestamp": event.timestamp,
                ":id": event.id,
            },
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

    /// Calls the provided function once for each event the database.
    ///
    /// # Errors
    ///
    /// Returns an error if preparing or executing the query fails, or if a row
    /// cannot be converted into an [`Event`].
    pub fn for_each<F>(&self, mut callback: F) -> Result<()>
    where
        F: FnMut(Event) -> Result<()>,
    {
        let mut statement = self.connection.prepare("SELECT * FROM event")?;

        let rows = statement.query_map([], Self::event_from_row)?;

        for event in rows {
            callback(event?)?;
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
    use std::error::Error;
    use crate::db::database::Database;
    use crate::db::event_repository::EventRepository;
    use crate::model::event::EventType::{Start, Stop};
    use rusqlite::Result;
    use crate::db::project_repository::ProjectRepository;

    #[test]
    fn test_session_duration_calculation() -> Result<(), Box<dyn Error>> {
        let database = Database::new_in_memory_db()?;
        database.init()?;

        let project_repository = ProjectRepository::new(database.connection());
        project_repository.insert("Test name", None)?;
        let event_repository = EventRepository::new(database.connection());

        let start_timestamp = 1_780_140_094;
        event_repository.insert(1, Start, start_timestamp)?;
        event_repository.insert(1, Stop, start_timestamp + 300)?;

        event_repository.for_each_session(Some(1), None, |session| {
            assert_eq!(session.project_id, 1);
            assert_eq!(session.total_seconds, 300);
            Ok(())
        })?;

        Ok(())
    }
}
