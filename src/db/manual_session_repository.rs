use crate::db::database::Repository;
use crate::model::manual_session::ManualSession;
use rusqlite::{Connection, Result, named_params};
use time::Date;

pub struct ManualSessionRepository<'a> {
    connection: &'a Connection,
}

impl<'a> ManualSessionRepository<'a> {
    #[must_use]
    pub fn new(connection: &'a Connection) -> Self {
        Self { connection }
    }

    /// Inserts a new or updates an existing manual session.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the insert statement, for
    /// example because the database connection is invalid, the `manual_session` table
    /// does not exist, or the provided data violates a database constraint.
    pub fn upsert(&self, project_id: i32, date: Date, total_seconds: i64) -> Result<()> {
        self.connection.execute(
            "INSERT INTO manual_session (project_id, date, total_seconds)
            VALUES (:project_id, :date, :total_seconds)
            ON CONFLICT(project_id, date)
            DO UPDATE SET total_seconds = excluded.total_seconds",
            named_params! {
                ":project_id": project_id,
                ":date": date.to_string(),
                ":total_seconds": total_seconds
            },
        )?;

        Ok(())
    }

    /// Deletes the manual session for the given project on the given date.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the delete statement, for
    /// example because the database connection is invalid or the `manual_session`
    /// table does not exist.
    pub fn delete(&self, project_id: i32, date: Date) -> Result<bool> {
        let deleted_count = self.connection.execute(
            "DELETE FROM manual_session
            WHERE project_id = :project_id AND date = :date",
            named_params! {":project_id": project_id, ":date": date.to_string()},
        )?;

        Ok(deleted_count > 0)
    }

    /// Calls the provided consumer once for each session matching the given date.
    ///
    /// # Errors
    ///
    /// Returns an error if preparing or executing the query fails, or if a returned
    /// row cannot be converted into a [`ManualSession`].
    pub fn for_each<F>(&self, date: Date, project_id: Option<i32>, mut consumer: F) -> Result<()>
    where
        F: FnMut(ManualSession),
    {
        let mut statement = self.connection.prepare(
            "SELECT project_id, total_seconds
            FROM manual_session
            WHERE
                date = :date AND
                (:project_id IS NULL OR project_id = :project_id)",
        )?;

        let rows = statement.query_map(
            named_params! {
                ":date": date.to_string(),
                ":project_id": project_id
            },
            |row| {
                Ok(ManualSession {
                    project_id: row.get("project_id")?,
                    date,
                    total_seconds: row.get("total_seconds")?,
                })
            },
        )?;

        for duration in rows {
            consumer(duration?);
        }

        Ok(())
    }
}

impl<'a> Repository<'a> for ManualSessionRepository<'a> {
    fn initialize_schema(connection: &'a Connection) -> Result<()> {
        connection.execute(
            "CREATE TABLE IF NOT EXISTS manual_session (
                project_id INTEGER NOT NULL,
                date TEXT NOT NULL,
                total_seconds INTEGER NOT NULL,
                PRIMARY KEY (project_id, date),
                FOREIGN KEY(project_id)
                    REFERENCES project(id)
                    ON DELETE CASCADE
            )",
            (),
        )?;

        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_manual_session_date ON manual_session(date)",
            (),
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::db_test_context::DBTestContext;
    use crate::db::project_repository::ProjectRepository;
    use time::Month;

    fn initialize_context() -> Result<DBTestContext> {
        let context = DBTestContext::new()?;
        let project_repository = ProjectRepository::new(context.connection());

        project_repository.insert("Test name", None)?;
        project_repository.insert("Another project", None)?;

        Ok(context)
    }

    #[test]
    fn test_upsert() -> Result<()> {
        let context = initialize_context()?;
        let repository = ManualSessionRepository::new(context.connection());
        let date_1 = Date::from_calendar_date(2026, Month::May, 5).expect("Could not create date");
        let date_2 = Date::from_calendar_date(2026, Month::May, 6).expect("Could not create date");

        repository.upsert(1, date_1, 5000)?;
        repository.upsert(2, date_1, 500)?;
        repository.upsert(1, date_2, 900)?;

        let mut sessions = context.collect_sessions()?;
        assert_eq!(sessions.len(), 3);

        let mut session_0 = &sessions[0];
        let mut session_1 = &sessions[1];
        let mut session_2 = &sessions[2];

        assert_eq!(session_0.project_id, 1);
        assert_eq!(session_0.total_seconds, 5000);
        assert_eq!(session_0.date.to_string(), "2026-05-05");

        assert_eq!(session_1.project_id, 2);
        assert_eq!(session_1.total_seconds, 500);
        assert_eq!(session_1.date.to_string(), "2026-05-05");

        assert_eq!(session_2.project_id, 1);
        assert_eq!(session_2.total_seconds, 900);
        assert_eq!(session_2.date.to_string(), "2026-05-06");

        repository.upsert(1, date_1, 6000)?;

        sessions = context.collect_sessions()?;
        assert_eq!(sessions.len(), 3);

        session_0 = &sessions[0];
        session_1 = &sessions[1];
        session_2 = &sessions[2];

        assert_eq!(session_0.project_id, 1);
        assert_eq!(session_0.total_seconds, 6000);
        assert_eq!(session_0.date.to_string(), "2026-05-05");

        assert_eq!(session_1.project_id, 2);
        assert_eq!(session_1.total_seconds, 500);
        assert_eq!(session_1.date.to_string(), "2026-05-05");

        assert_eq!(session_2.project_id, 1);
        assert_eq!(session_2.total_seconds, 900);
        assert_eq!(session_2.date.to_string(), "2026-05-06");

        Ok(())
    }

    #[test]
    fn test_delete_existing_session() -> Result<()> {
        let context = initialize_context()?;
        let repository = ManualSessionRepository::new(context.connection());
        let date = Date::from_calendar_date(2026, Month::May, 5).expect("Could not create date");

        repository.upsert(1, date, 1000)?;

        let deleted = repository.delete(1, date)?;
        assert!(deleted);

        let sessions = context.collect_sessions()?;
        assert!(sessions.is_empty());

        Ok(())
    }

    #[test]
    fn test_delete_non_existing_session() -> Result<()> {
        let context = initialize_context()?;
        let repository = ManualSessionRepository::new(context.connection());
        let date = Date::from_calendar_date(2026, Month::May, 5).expect("Could not create date");

        let deleted = repository.delete(1, date)?;
        assert!(!deleted);

        Ok(())
    }

    #[test]
    fn test_upsert_then_delete_then_reinsert() -> Result<()> {
        let context = initialize_context()?;
        let repository = ManualSessionRepository::new(context.connection());

        let date = Date::from_calendar_date(2026, Month::May, 5).expect("Could not create date");

        repository.upsert(1, date, 1000)?;
        repository.upsert(2, date, 2000)?;

        repository.delete(1, date)?;

        let sessions = context.collect_sessions()?;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].project_id, 2);

        repository.upsert(1, date, 3000)?;

        let sessions = context.collect_sessions()?;
        assert_eq!(sessions.len(), 2);

        Ok(())
    }
}
