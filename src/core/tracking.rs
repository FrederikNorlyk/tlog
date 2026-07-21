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
        }

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

    /// Start or stop time tracking on a project, depending on whether it is already started or not.
    ///
    /// # Errors
    ///
    /// Returns an error if starting or stopping events fails.
    pub fn toggle(&self, project_id: i32) -> Result<(), TrackingError> {
        let event_repository = EventRepository::new(self.connection);

        if let Some(started_event) = event_repository.get_started_event()?
            && started_event.project_id == project_id
        {
            self.stop(project_id)?;
        } else {
            self.start(project_id)?;
        }

        Ok(())
    }

    /// Explicitly sets time spent on a given project on a given date.
    ///
    /// The function deletes any existing events and creates a manual session.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the queries.
    pub fn set(
        &self,
        project_id: i32,
        date: Date,
        total_seconds: i64,
    ) -> Result<(), TrackingError> {
        self.overwrite_day(project_id, date, Some(total_seconds))?;

        Ok(())
    }

    /// Reset time tracking on the given project on the given date.
    ///
    /// It does so by removing any events and manual sessions on the given project on the given date.
    ///
    /// # Errors
    ///
    /// Returns an error if deleting or creating events in the database fails.
    pub fn reset(&self, project_id: i32, date: Date) -> Result<(), TrackingError> {
        self.overwrite_day(project_id, date, None)?;

        Ok(())
    }

    /// Adjusts the recorded total time for a specific project's sessions to the nearest rounded
    /// time (15-minute intervals)
    ///
    /// # Errors
    ///
    /// This method will return a `rusqlite::Error` in cases such as:
    /// * Failure to retrieve sessions from the database.
    /// * Failure to delete events from the `EventRepository`.
    /// * Failure to upsert the adjusted session in the `ManualSessionRepository`.
    pub fn adjust_by_fifteen_minutes(
        &self,
        project_id: i32,
        date: Date,
        operation: TimeAdjustmentOperation,
    ) -> Result<(), TrackingError> {
        const FIFTEEN_MINUTES_SECONDS: i64 = 15 * 60;

        let sessions = self.list_all_sessions(date, Some(project_id))?;
        let total_seconds = sessions.first().map_or(0, |s| s.total_seconds);

        let rounded_seconds = match operation {
            TimeAdjustmentOperation::Increment => {
                ((total_seconds / FIFTEEN_MINUTES_SECONDS) + 1) * FIFTEEN_MINUTES_SECONDS
            }
            TimeAdjustmentOperation::Decrement => {
                ((total_seconds - 1) / FIFTEEN_MINUTES_SECONDS) * FIFTEEN_MINUTES_SECONDS
            }
        }
        .max(0);

        self.overwrite_day(project_id, date, Some(rounded_seconds))?;

        Ok(())
    }

    fn overwrite_day(
        &self,
        project_id: i32,
        date: Date,
        total_seconds: Option<i64>,
    ) -> Result<(), TrackingError> {
        let event_repository = EventRepository::new(self.connection);
        let manual_session_repository = ManualSessionRepository::new(self.connection);

        event_repository.delete_all_in(project_id, date)?;

        // TODO: If a start event exists on the project on a previous date we need to handle it

        if let Some(seconds) = total_seconds {
            manual_session_repository.upsert(project_id, date, seconds)?;
        } else {
            manual_session_repository.delete(project_id, date)?;
        }

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
    pub fn list_all_sessions(
        &self,
        date: Date,
        project_id: Option<i32>,
    ) -> rusqlite::Result<Vec<Session>> {
        struct PrimitiveSession {
            project_id: i32,
            total_seconds: i64,
            is_started: bool,
        }

        let manual_sessions_repository = ManualSessionRepository::new(self.connection);
        let event_repository = EventRepository::new(self.connection);
        let project_repository = ProjectRepository::new(self.connection);

        let mut project_to_primitive_sessions = HashMap::new();

        manual_sessions_repository.for_each(date, project_id, |manual_session| {
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

        event_repository.for_each(date, project_id, |event| {
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

            if let Some(session) = project_to_primitive_sessions.get_mut(project_id) {
                session.total_seconds += total_seconds;
                session.is_started = is_started;
            } else {
                let session = PrimitiveSession {
                    project_id: *project_id,
                    total_seconds,
                    is_started,
                };
                project_to_primitive_sessions.insert(*project_id, session);
            }
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

        sessions.sort_by(|a, b| a.project.name.cmp(&b.project.name));

        Ok(sessions)
    }
}

#[derive(Copy, Clone)]
pub enum TimeAdjustmentOperation {
    Increment,
    Decrement,
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
    use crate::db::db_test_context::DBTestContext;
    use crate::db::event_repository::EventRepository;
    use crate::db::project_repository::ProjectRepository;
    use crate::model::event::EventType::{Start, Stop};
    use std::error::Error;
    use time::{OffsetDateTime, PrimitiveDateTime, Time};

    fn initialize_context() -> rusqlite::Result<DBTestContext> {
        let context = DBTestContext::new()?;
        let project_repository = ProjectRepository::new(context.connection());

        project_repository.insert("A", None)?;
        project_repository.insert("B", Some("A desc"))?;
        project_repository.insert("C", None)?;
        project_repository.insert("D", None)?;
        project_repository.insert("E", None)?;

        Ok(context)
    }

    mod start_stop {
        use super::*;

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
        fn start_stops_existing_started_events_before_starting_new_project() -> rusqlite::Result<()>
        {
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
    }

    mod toggle {
        use super::*;

        #[test]
        fn on_off() -> Result<(), Box<dyn Error>> {
            let context = initialize_context()?;
            let tracking = Tracking::new(context.connection());

            tracking.toggle(1)?;

            let events = context.collect_events()?;
            assert_eq!(events.len(), 1);

            let event = events.first().unwrap();
            assert_eq!(event.project_id, 1);
            assert_eq!(event.event_type, Start);

            tracking.toggle(1)?;

            let events = context.collect_events()?;
            assert_eq!(events.len(), 2);

            let event = events.first().unwrap();
            assert_eq!(event.project_id, 1);
            assert_eq!(event.event_type, Start);

            let event = events.get(1).unwrap();
            assert_eq!(event.project_id, 1);
            assert_eq!(event.event_type, Stop);

            Ok(())
        }

        #[test]
        fn several_projects() -> Result<(), Box<dyn Error>> {
            let context = initialize_context()?;
            let tracking = Tracking::new(context.connection());

            tracking.toggle(1)?;

            let events = context.collect_events()?;
            assert_eq!(events.len(), 1);

            let event = events.first().unwrap();
            assert_eq!(event.project_id, 1);
            assert_eq!(event.event_type, Start);

            tracking.toggle(2)?;

            let events = context.collect_events()?;
            assert_eq!(events.len(), 3);

            let event = events.first().unwrap();
            assert_eq!(event.project_id, 1);
            assert_eq!(event.event_type, Start);

            let event = events.get(1).unwrap();
            assert_eq!(event.project_id, 1);
            assert_eq!(event.event_type, Stop);

            let event = events.get(2).unwrap();
            assert_eq!(event.project_id, 2);
            assert_eq!(event.event_type, Start);

            Ok(())
        }
    }

    mod set {
        use super::*;

        #[test]
        fn test_set_manual_session_deletes_events() -> Result<(), Box<dyn Error>> {
            let context = initialize_context()?;
            let tracking = Tracking::new(context.connection());
            let event_repository = EventRepository::new(context.connection());

            let date = Date::from_calendar_date(2026, time::Month::June, 3)?;
            let time = Time::from_hms(10, 0, 0)?;

            let mut timestamp = PrimitiveDateTime::new(date, time)
                .assume_utc()
                .unix_timestamp();

            // -------------------------
            // Create start stop events for project 1 and 2
            // -------------------------
            event_repository.insert(1, Start, timestamp)?;
            timestamp += 10;
            event_repository.insert(1, Stop, timestamp)?;
            timestamp += 1000;
            event_repository.insert(2, Start, timestamp)?;
            timestamp += 50;
            event_repository.insert(2, Stop, timestamp)?;

            // -------------------------
            // Assert event-based sessions duration
            // -------------------------
            let sessions = tracking.list_all_sessions(date, None)?;
            assert_eq!(sessions.len(), 2);

            let session = sessions.first().unwrap();
            assert_eq!(session.project.id, 1);
            assert_eq!(session.total_seconds, 10);

            let session = sessions.get(1).unwrap();
            assert_eq!(session.project.id, 2);
            assert_eq!(session.total_seconds, 50);

            // -------------------------
            // Manually set project 1 to 200 seconds
            // -------------------------
            tracking.set(1, date, 200)?;

            // -------------------------
            // Verify that project 1 is 200 seconds, and project 2 is still event-based
            // -------------------------
            let sessions = tracking.list_all_sessions(date, None)?;
            assert_eq!(sessions.len(), 2);

            let session = sessions.first().unwrap();
            assert_eq!(session.project.id, 1);
            assert_eq!(session.total_seconds, 200);

            let session = sessions.get(1).unwrap();
            assert_eq!(session.project.id, 2);
            assert_eq!(session.total_seconds, 50);

            Ok(())
        }
    }

    mod reset {
        use super::*;

        #[test]
        fn removes_events_and_manual_sessions_for_given_date() -> Result<(), Box<dyn Error>> {
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
            // Other projects' events should not be deleted
            event_repository.insert(2, Start, timestamp)?;

            let next_day =
                Date::from_calendar_date(2026, time::Month::June, 3).expect("valid date");

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
    }

    mod adjust_by_fifteen_minutes {
        use super::*;
        use crate::core::tracking::TimeAdjustmentOperation::Decrement;
        use crate::core::tracking::TimeAdjustmentOperation::Increment;

        #[test]
        fn rounds_to_nearest_quarter() -> Result<(), Box<dyn Error>> {
            let context = initialize_context()?;
            let tracking = Tracking::new(context.connection());
            let date = Date::from_calendar_date(2024, time::Month::April, 11)?;

            tracking.set(1, date, 400)?;
            tracking.set(2, date, 400)?;

            // Rounding up to nearest quarter (900 seconds)
            tracking.adjust_by_fifteen_minutes(1, date, Increment)?;
            // Rounding down to nearest quarter (0 seconds)
            tracking.adjust_by_fifteen_minutes(2, date, Decrement)?;

            let sessions = tracking.list_all_sessions(date, None)?;
            let first = sessions.first().unwrap();
            let second = sessions.get(1).unwrap();

            assert_eq!(first.project.id, 1);
            assert!(!first.is_started);
            assert_eq!(first.total_seconds, 900);

            assert_eq!(second.project.id, 2);
            assert!(!second.is_started);
            assert_eq!(second.total_seconds, 0);

            // Rounding up to 1,800 seconds
            tracking.adjust_by_fifteen_minutes(1, date, Increment)?;
            // Rounding up to 2,700 seconds
            tracking.adjust_by_fifteen_minutes(1, date, Increment)?;
            // Rounding down to 1,800 seconds
            tracking.adjust_by_fifteen_minutes(1, date, Decrement)?;

            let sessions = tracking.list_all_sessions(date, None)?;
            let first = sessions.first().unwrap();
            let second = sessions.get(1).unwrap();

            assert_eq!(first.project.id, 1);
            assert!(!first.is_started);
            assert_eq!(first.total_seconds, 1800);

            assert_eq!(second.project.id, 2);
            assert!(!second.is_started);
            assert_eq!(second.total_seconds, 0);

            Ok(())
        }

        #[test]
        fn zero_is_lowest_value() -> Result<(), Box<dyn Error>> {
            let context = initialize_context()?;
            let tracking = Tracking::new(context.connection());
            let date = Date::from_calendar_date(2024, time::Month::April, 11)?;

            tracking.set(1, date, 400)?;
            // Rounding down to nearest quarter (0 seconds)
            tracking.adjust_by_fifteen_minutes(1, date, Decrement)?;
            // Rounding down again stays at 0 seconds.
            tracking.adjust_by_fifteen_minutes(1, date, Decrement)?;

            let sessions = tracking.list_all_sessions(date, None)?;
            let session = sessions.first().unwrap();

            assert_eq!(session.project.id, 1);
            assert!(!session.is_started);
            assert_eq!(session.total_seconds, 0);

            Ok(())
        }

        #[test]
        fn test_ongoing_sessions() -> Result<(), Box<dyn Error>> {
            let context = initialize_context()?;
            let tracking = Tracking::new(context.connection());
            let date = OffsetDateTime::now_utc().date();

            // Manual session (project 1)
            tracking.set(1, date, 400)?;

            // Ongoing session (project 2)
            tracking.toggle(2)?;

            // Adjusting a session does not modify the ongoing session
            tracking.adjust_by_fifteen_minutes(1, date, Increment)?;

            let sessions = tracking.list_all_sessions(date, None)?;
            let first = sessions.first().unwrap();
            let second = sessions.get(1).unwrap();

            assert_eq!(first.project.id, 1);
            assert!(!first.is_started);
            assert_eq!(first.total_seconds, 900);

            assert_eq!(second.project.id, 2);
            assert!(second.is_started);
            assert_eq!(second.total_seconds, 0);

            // Adjusting an ongoing session stops it.
            tracking.adjust_by_fifteen_minutes(2, date, Increment)?;

            let sessions = tracking.list_all_sessions(date, None)?;
            let first = sessions.first().unwrap();
            let second = sessions.get(1).unwrap();

            assert_eq!(first.project.id, 1);
            assert!(!first.is_started);
            assert_eq!(first.total_seconds, 900);

            assert_eq!(second.project.id, 2);
            assert!(!second.is_started);
            assert_eq!(second.total_seconds, 900);

            Ok(())
        }
    }

    mod list_all_sessions {
        use super::*;

        #[test]
        fn test() -> Result<(), Box<dyn Error>> {
            let context = initialize_context()?;
            let tracking = Tracking::new(context.connection());
            let event_repository = EventRepository::new(context.connection());
            let manual_session_repository = ManualSessionRepository::new(context.connection());

            let now = OffsetDateTime::now_utc();
            let today = now.date();
            let today_timestamp = now.unix_timestamp();

            let date = Date::from_calendar_date(2026, time::Month::June, 3)?;
            let time = Time::from_hms(10, 0, 0)?;

            let mut timestamp = PrimitiveDateTime::new(date, time)
                .assume_utc()
                .unix_timestamp();

            // -------------------------
            // Manual session (project 1)
            // -------------------------
            manual_session_repository.upsert(1, date, 3600)?;

            // -------------------------
            // Event session (project 2)
            // -------------------------
            event_repository.insert(2, Start, timestamp)?;
            timestamp += 10;
            event_repository.insert(2, Stop, timestamp)?;
            timestamp += 10;
            event_repository.insert(2, Start, timestamp)?;
            timestamp += 10;
            event_repository.insert(2, Stop, timestamp)?;

            // -------------------------
            // Combination of manual and event session (project 3)
            // -------------------------
            tracking.set(3, date, 1000)?;
            event_repository.insert(3, Start, timestamp)?;
            timestamp += 10;
            event_repository.insert(3, Stop, timestamp)?;

            // -------------------------
            // Ongoing session (project 4): Start only
            // -------------------------
            event_repository.insert(4, Start, today_timestamp - 100)?;

            // -------------------------
            // Execute
            // -------------------------
            let sessions = tracking.list_all_sessions(date, None)?;
            let today_sessions = tracking.list_all_sessions(today, None)?;

            // -------------------------
            // Assertions
            // -------------------------
            assert_eq!(sessions.len(), 3);
            assert_eq!(today_sessions.len(), 1);

            // Project 1: manual session
            let session = sessions.first().unwrap();
            assert_eq!(session.project.id, 1);
            assert_eq!(session.project.name, "A");
            assert!(session.project.description.is_none());
            assert_eq!(session.total_seconds, 3600);
            assert!(!session.is_started);

            // Project 2: event session
            let session = sessions.get(1).unwrap();
            assert_eq!(session.project.id, 2);
            assert_eq!(session.project.name, "B");
            assert_eq!(session.project.description, Some("A desc".to_string()));
            assert_eq!(session.total_seconds, 20);
            assert!(!session.is_started);

            // Project 3: manual and event session
            let session = sessions.get(2).unwrap();
            assert_eq!(session.project.id, 3);
            assert_eq!(session.project.name, "C");
            assert!(session.project.description.is_none());
            assert!(!session.is_started);
            assert_eq!(1010, session.total_seconds);

            // Ongoing sessions use current time, so we can't be 100% sure
            let min_duration = 100;
            // Should never take more than 10 seconds to execute the test
            let max_duration = 110;

            // Project 4: ongoing session (Start only)
            let session = today_sessions.first().unwrap();
            assert_eq!(session.project.id, 4);
            assert_eq!(session.project.name, "D");
            assert!(session.project.description.is_none());
            assert!(session.is_started);

            assert!(
                session.total_seconds >= min_duration && session.total_seconds <= max_duration,
                "total_seconds: {}",
                session.total_seconds
            );

            Ok(())
        }

        #[test]
        fn combined_ongoing_and_manual_session() -> Result<(), Box<dyn Error>> {
            let context = initialize_context()?;
            let tracking = Tracking::new(context.connection());
            let event_repository = EventRepository::new(context.connection());

            let now = OffsetDateTime::now_utc();
            let today = now.date();
            let today_timestamp = now.unix_timestamp();

            // -------------------------
            // Combination of manual session and ongoing event session (project 5)
            // -------------------------
            tracking.set(5, today, 1000)?;
            event_repository.insert(5, Start, today_timestamp - 100)?;

            // -------------------------
            // Execute
            // -------------------------
            let today_sessions = tracking.list_all_sessions(today, None)?;

            // -------------------------
            // Assertions
            // -------------------------
            assert_eq!(today_sessions.len(), 1);

            // Ongoing sessions use current time, so we can't be 100% sure
            let min_duration = 1100;
            // Should never take more than 10 seconds to execute the test
            let max_duration = 1110;

            // Project 5: manual session + ongoing session
            let session = today_sessions.first().unwrap();
            assert_eq!(session.project.id, 5);
            assert_eq!(session.project.name, "E");
            assert!(session.project.description.is_none());
            assert!(session.is_started);

            assert!(
                session.total_seconds >= min_duration && session.total_seconds <= max_duration,
                "total_seconds: {}",
                session.total_seconds
            );

            Ok(())
        }
    }
}
