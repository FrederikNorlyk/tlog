use std::time::Duration;
use time::{OffsetDateTime, UtcOffset, macros::datetime};
use tlog::core::tracking::Tracking;
use tlog::db::{
    database::Database, event_repository::EventRepository,
    manual_session_repository::ManualSessionRepository, project_repository::ProjectRepository,
};
use tlog::model::{event::EventType, ids::EventId};

#[test]
fn event_storage_normalizes_offsets_and_preserves_whole_seconds() {
    let database = Database::new_in_memory_db().unwrap();
    database.init().unwrap();
    let connection = database.connection();
    let project_id = ProjectRepository::new(connection)
        .insert("Test", None)
        .unwrap();
    let events = EventRepository::new(connection);
    let start = datetime!(2026-09-27 10:00:00.750 +02:00);
    let stop = datetime!(2026-09-27 09:30:00.250 +01:00);
    events.insert(project_id, EventType::Start, start).unwrap();
    events.insert(project_id, EventType::Stop, stop).unwrap();

    let stored = events.get(EventId(1)).unwrap().unwrap();
    assert_eq!(stored.timestamp, start.truncate_to_second());
    assert_eq!(stored.timestamp.offset(), UtcOffset::UTC);
    let raw: i64 = connection
        .query_row("SELECT timestamp FROM event WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(raw, start.unix_timestamp());

    let sessions = Tracking::new(connection)
        .list_all_sessions(start.date(), None)
        .unwrap();
    assert_eq!(sessions[0].duration, Duration::from_mins(30));
}

#[test]
fn manual_durations_keep_existing_integer_storage() {
    let database = Database::new_in_memory_db().unwrap();
    database.init().unwrap();
    let connection = database.connection();
    let project_id = ProjectRepository::new(connection)
        .insert("Test", None)
        .unwrap();
    let date = datetime!(2026-09-27 00:00 UTC).date();
    let manual = ManualSessionRepository::new(connection);
    for seconds in [0, 90, 3600] {
        manual
            .upsert(project_id, date, Duration::from_secs(seconds))
            .unwrap();
        let raw: i64 = connection
            .query_row("SELECT total_seconds FROM manual_session", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(u64::try_from(raw).unwrap(), seconds);
        let sessions = Tracking::new(connection)
            .list_all_sessions(date, None)
            .unwrap();
        assert_eq!(sessions[0].duration, Duration::from_secs(seconds));
    }
    manual
        .upsert(project_id, date, Duration::from_millis(1999))
        .unwrap();
    let sessions = Tracking::new(connection)
        .list_all_sessions(date, None)
        .unwrap();
    assert_eq!(sessions[0].duration, Duration::from_secs(1));
}

#[test]
fn existing_timestamps_are_read_and_out_of_range_values_return_errors() {
    let database = Database::new_in_memory_db().unwrap();
    database.init().unwrap();
    let connection = database.connection();
    let project_id = ProjectRepository::new(connection)
        .insert("Test", None)
        .unwrap();
    connection
        .execute(
            "INSERT INTO event (id, project_id, event_type, timestamp) VALUES (1, ?1, 0, -1)",
            [project_id],
        )
        .unwrap();
    let events = EventRepository::new(connection);
    assert_eq!(
        events.get(EventId(1)).unwrap().unwrap().timestamp,
        OffsetDateTime::UNIX_EPOCH - Duration::from_secs(1),
    );

    connection
        .execute("UPDATE event SET timestamp = ?1", [i64::MAX])
        .unwrap();
    let error = events.get(EventId(1)).unwrap_err();
    assert!(matches!(
        error,
        rusqlite::Error::FromSqlConversionFailure(_, _, _)
    ));
}

#[test]
fn negative_stored_durations_are_rejected() {
    let database = Database::new_in_memory_db().unwrap();
    database.init().unwrap();
    let connection = database.connection();
    let project_id = ProjectRepository::new(connection)
        .insert("Test", None)
        .unwrap();
    let date = datetime!(2026-09-27 00:00 UTC).date();
    connection
        .execute(
            "INSERT INTO manual_session (project_id, date, total_seconds) VALUES (?1, ?2, -90)",
            rusqlite::params![project_id, date.to_string()],
        )
        .unwrap();
    let error = Tracking::new(connection)
        .list_all_sessions(date, None)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        tlog::core::tracking::TrackingError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
            _,
            _,
            _
        ))
    ));
}

#[test]
fn a_future_start_returns_an_error_instead_of_a_negative_duration() {
    let database = Database::new_in_memory_db().unwrap();
    database.init().unwrap();
    let connection = database.connection();
    let project_id = ProjectRepository::new(connection)
        .insert("Test", None)
        .unwrap();
    let start = OffsetDateTime::now_utc() + Duration::from_hours(1);
    EventRepository::new(connection)
        .insert(project_id, EventType::Start, start)
        .unwrap();
    let error = Tracking::new(connection)
        .list_all_sessions(start.date(), None)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        tlog::core::tracking::TrackingError::ReversedTimestamps { .. }
    ));
}

#[test]
fn an_unstorable_duration_does_not_delete_existing_events() {
    let database = Database::new_in_memory_db().unwrap();
    database.init().unwrap();
    let connection = database.connection();
    let project_id = ProjectRepository::new(connection)
        .insert("Test", None)
        .unwrap();
    let start = datetime!(2026-09-27 00:00 UTC);
    let events = EventRepository::new(connection);
    events.insert(project_id, EventType::Start, start).unwrap();
    let duration = Duration::from_secs(u64::MAX);
    assert!(
        Tracking::new(connection)
            .set(project_id, start.date(), duration)
            .is_err()
    );
    assert!(events.get(EventId(1)).unwrap().is_some());
    assert!(
        ManualSessionRepository::new(connection)
            .upsert(project_id, start.date(), duration)
            .is_err()
    );
}

#[test]
fn input_parsers_reject_negative_and_out_of_range_durations() {
    use clap::Parser;
    use tlog::cli::commands::{Cli, Command};
    use tlog::core::time_format::TimeFormat;

    for text in ["-1:00", "1:-1", "18446744073709551615:00"] {
        let argument = format!("--duration={text}");
        assert!(Cli::try_parse_from(["tlog", "set", "-p", "1", &argument]).is_err());
    }
    let cli = Cli::try_parse_from(["tlog", "set", "-p", "1", "--duration=1:30"]).unwrap();
    assert!(
        matches!(cli.command, Some(Command::Set { duration, .. }) if duration == Duration::from_mins(90))
    );

    for (format, invalid) in [
        (TimeFormat::Seconds, vec!["-1", "18446744073709551616"]),
        (
            TimeFormat::HoursMinutes,
            vec!["-1:00", "1:-1", "18446744073709551615:00"],
        ),
        (
            TimeFormat::HoursMinutesSeconds,
            vec![
                "-1:00:00",
                "1:-1:00",
                "1:00:-1",
                "18446744073709551615:00:00",
            ],
        ),
        (
            TimeFormat::DecimalHours,
            vec!["-1.5", "1:-1", "NaN", "inf", "1e100"],
        ),
    ] {
        for input in invalid {
            assert!(format.parse(input).is_err(), "{format:?} accepted {input}");
        }
        assert_eq!(format.parse("0").unwrap(), Duration::ZERO);
        // Even the largest standard duration must be safe to round for display.
        let _ = format.round(Duration::MAX);
    }
    assert_eq!(
        TimeFormat::DecimalHours.parse("1.5").unwrap(),
        Duration::from_mins(90)
    );
}
