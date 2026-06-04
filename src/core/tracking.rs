use crate::core::unix_timestamp::UnixTimestamp;
use crate::db::event_repository::EventRepository;
use crate::db::manual_session_repository::ManualSessionRepository;
use crate::db::project_repository::ProjectRepository;
use crate::model::event::{Event, EventType};
use crate::model::project::Project;
use crate::model::session::Session;
use rusqlite::Connection;
use std::collections::HashMap;
use thiserror::Error;
use time::Date;

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

        if let Some(started_event) = event_repository.get_started_event()? {
            event_repository.insert(started_event.project_id, EventType::Stop, timestamp)?;
        };

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

    /// Reset time tracking on the given project on the given date.
    ///
    /// It does so by remove any events and manual sessions on the given project on the given date.
    ///
    /// # Errors
    ///
    /// Returns an error if deleting or creating events in the database fails.
    pub fn reset(&self, project_id: i32, date: Date) -> rusqlite::Result<()> {
        let event_repository = EventRepository::new(self.connection);
        event_repository.delete_all_in(project_id, date)?;

        let manual_session_repository = ManualSessionRepository::new(self.connection);
        manual_session_repository.delete(project_id, date)?;

        Ok(())
    }

    /// Returns aggregated sessions for a given date.
    ///
    /// Sessions are built from either manual sessions or event logs.
    /// Event sessions are computed from Start/Stop pairs, with running time added
    /// if the last event is a Start.
    ///
    /// Manual sessions override event-based sessions for the same project.
    ///
    /// # Errors
    ///
    /// Returns a database error if any query or project lookup fails.
    pub fn list_all_sessions(&self, date: Date) -> rusqlite::Result<Vec<Session>> {
        struct PrimitiveSession {
            project_id: i32,
            total_seconds: i64,
            is_started: bool,
        }

        let manual_sessions_repository = ManualSessionRepository::new(self.connection);
        let event_repository = EventRepository::new(self.connection);
        let project_repository = ProjectRepository::new(self.connection);

        let mut project_to_primitive_sessions = HashMap::new();

        manual_sessions_repository.for_each(date, |manual_session| {
            project_to_primitive_sessions.insert(
                manual_session.project_id,
                PrimitiveSession {
                    project_id: manual_session.project_id,
                    total_seconds: manual_session.total_seconds,
                    is_started: false,
                },
            );
        })?;

        let mut project_to_events: HashMap<i32, Vec<Event>> = HashMap::new();

        event_repository.for_each(date, |event| {
            // Overwritten by a manual session
            if project_to_primitive_sessions.contains_key(&event.project_id) {
                return;
            }

            project_to_events
                .entry(event.project_id)
                .or_default()
                .push(event);
        })?;

        let started_event = event_repository.get_started_event()?;

        for (project_id, events) in &project_to_events {
            let mut i = 0;
            let mut total_seconds = 0;

            while i + 1 < events.len() {
                let start = &events[i];
                let stop = &events[i + 1];

                total_seconds += stop.timestamp - start.timestamp;

                i += 2;
            }

            let mut is_started = false;

            if let Some(started_event) = &started_event
                && started_event.project_id == *project_id
            {
                total_seconds += UnixTimestamp::now() - started_event.timestamp;
                is_started = true;
            }

            let session = PrimitiveSession {
                project_id: *project_id,
                total_seconds,
                is_started,
            };

            project_to_primitive_sessions.insert(*project_id, session);
        }

        let project_ids: Vec<i32> = project_to_primitive_sessions.keys().copied().collect();

        let projects = project_repository.find_by_ids(&project_ids)?;

        let mut project_id_to_project: HashMap<i32, Project> =
            projects.into_iter().map(|p| (p.id, p)).collect();

        let mut sessions = Vec::new();

        for primitive_session in project_to_primitive_sessions.values() {
            let Some(project) = project_id_to_project.remove(&primitive_session.project_id) else {
                continue;
            };

            sessions.push(Session {
                project,
                total_seconds: primitive_session.total_seconds,
                is_started: primitive_session.is_started,
            });
        }

        Ok(sessions)
    }
}

#[derive(Debug, Error)]
pub enum TrackingError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("no active start event for project {project_id}")]
    NoActiveStartEvent { project_id: i32 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::event_repository::EventRepository;
    use crate::db::project_repository::ProjectRepository;
    use crate::db::test_utils::DBTestContext;
    use crate::model::event::EventType::{Start, Stop};
    use std::error::Error;
    use time::{PrimitiveDateTime, Time};

    fn initialize_context() -> rusqlite::Result<DBTestContext> {
        let context = DBTestContext::new()?;
        let project_repository = ProjectRepository::new(context.connection());

        project_repository.insert("First", None)?;
        project_repository.insert("Second", Some("A desc"))?;
        project_repository.insert("Third", None)?;

        Ok(context)
    }

    #[test]
    fn test_start_and_stop() -> Result<(), TrackingError> {
        let context = initialize_context()?;

        let tracking = Tracking::new(context.connection());

        tracking.start(1)?;
        tracking.stop(1)?;

        let events = context.collect_events()?;

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
        let context = initialize_context()?;
        let tracking = Tracking::new(context.connection());

        tracking.start(1)?;
        tracking.start(2)?;

        let events = context.collect_events()?;

        assert_eq!(events.len(), 3);

        assert_eq!(events[0].project_id, 1);
        assert!(matches!(events[0].event_type, Start));

        assert_eq!(events[1].project_id, 1);
        assert!(matches!(events[1].event_type, Stop));

        assert_eq!(events[2].project_id, 2);
        assert!(matches!(events[2].event_type, Start));

        Ok(())
    }

    #[test]
    fn stop_without_start_will_fail() -> Result<(), TrackingError> {
        let context = initialize_context()?;
        let tracking = Tracking::new(context.connection());
        let result = tracking.stop(1);

        assert!(matches!(
            result,
            Err(TrackingError::NoActiveStartEvent { project_id }) if project_id == 1
        ));

        Ok(())
    }

    #[test]
    fn reset_removes_events_and_manual_sessions_for_given_date() -> Result<(), Box<dyn Error>> {
        let context = initialize_context()?;
        let tracking = Tracking::new(context.connection());
        let event_repository = EventRepository::new(context.connection());
        let manual_session_repository = ManualSessionRepository::new(context.connection());

        let date = Date::from_calendar_date(2026, time::Month::June, 2).expect("valid date");
        let time = Time::from_hms(15, 56, 31)?;

        let mut timestamp = PrimitiveDateTime::new(date, time)
            .assume_utc()
            .unix_timestamp();

        event_repository.insert(1, Start, timestamp)?;
        timestamp += 10;
        event_repository.insert(1, Stop, timestamp)?;
        timestamp += 10;
        // Other project's events should not be deleted
        event_repository.insert(2, Start, timestamp)?;

        let next_day = Date::from_calendar_date(2026, time::Month::June, 3).expect("valid date");
        let next_day_timestamp = PrimitiveDateTime::new(next_day, time)
            .assume_utc()
            .unix_timestamp();

        // Events on other days should not be deleted
        event_repository.insert(1, Start, next_day_timestamp)?;

        manual_session_repository.upsert(1, date, 3600)?;

        let events_before = context.collect_events()?;
        assert_eq!(events_before.len(), 4);

        let sessions_before = context.collect_sessions()?;
        assert_eq!(sessions_before.len(), 1);

        tracking.reset(1, date)?;

        let events_after = context.collect_events()?;
        assert_eq!(events_after.len(), 2);

        let event_0 = &events_after[0];
        assert_eq!(event_0.project_id, 2);
        let event_1 = &events_after[1];
        assert_eq!(event_1.project_id, 1);
        assert_eq!(event_1.timestamp, next_day_timestamp);

        let sessions_after = context.collect_sessions()?;
        assert!(sessions_after.is_empty());

        Ok(())
    }

    #[test]
    fn list_all_sessions_combines_manual_and_event_sessions() -> Result<(), Box<dyn Error>> {
        let context = initialize_context()?;
        let tracking = Tracking::new(context.connection());
        let event_repository = EventRepository::new(context.connection());
        let manual_session_repository = ManualSessionRepository::new(context.connection());

        let date = Date::from_calendar_date(2026, time::Month::June, 3)?;

        // -------------------------
        // Manual session (project 1)
        // -------------------------
        manual_session_repository.upsert(1, date, 3600)?;

        // -------------------------
        // Event session (project 2): 10s + 20s = 30s
        // -------------------------
        let time = Time::from_hms(10, 0, 0)?;

        let mut timestamp = PrimitiveDateTime::new(date, time)
            .assume_utc()
            .unix_timestamp();

        event_repository.insert(2, Start, timestamp)?;
        timestamp += 10;
        event_repository.insert(2, Stop, timestamp)?;
        timestamp += 10;
        event_repository.insert(2, Start, timestamp)?;
        timestamp += 10;
        event_repository.insert(2, Stop, timestamp)?;

        // -------------------------
        // Ongoing session (project 3): Start only
        // -------------------------
        event_repository.insert(3, Start, timestamp + 100)?;

        // -------------------------
        // Execute
        // -------------------------
        let sessions = tracking.list_all_sessions(date)?;

        // -------------------------
        // Assertions
        // -------------------------
        assert_eq!(sessions.len(), 3);

        let mut map = sessions
            .into_iter()
            .map(|s| (s.project.id, s))
            .collect::<HashMap<_, _>>();

        // Project 1: manual session
        let s1 = map.remove(&1).unwrap();
        assert_eq!(s1.project.id, 1);
        assert_eq!(s1.project.name, "First");
        assert!(s1.project.description.is_none());
        assert_eq!(s1.total_seconds, 3600);
        assert!(!s1.is_started);

        // Project 2: event session
        let s2 = map.remove(&2).unwrap();
        assert_eq!(s2.project.id, 2);
        assert_eq!(s2.project.name, "Second");
        assert_eq!(s2.project.description, Some("A desc".to_string()));
        assert_eq!(s2.total_seconds, 20);
        assert!(!s2.is_started);

        // Project 3: ongoing session (Start only)
        let s3 = map.remove(&3).unwrap();
        assert_eq!(s3.project.id, 3);
        assert_eq!(s3.project.name, "Third");
        assert!(s3.project.description.is_none());
        assert!(s3.is_started);
        assert!(s3.total_seconds >= 0);

        Ok(())
    }
}
