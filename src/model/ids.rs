use std::{fmt, num::ParseIntError, str::FromStr};

use rusqlite::{
    ToSql,
    types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef},
};

/// Identifies a project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProjectId(pub i64);

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ProjectId {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

impl ToSql for ProjectId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for ProjectId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}

/// Identifies an event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventId(pub i64);

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for EventId {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

impl ToSql for EventId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for EventId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::commands::{Cli, Command};
    use crate::db::database::Database;
    use crate::db::event_repository::EventRepository;
    use crate::db::project_repository::ProjectRepository;
    use clap::Parser;

    #[test]
    fn ids_above_i32_range_round_trip_through_repositories() {
        let database = Database::new_in_memory_db().unwrap();
        database.init().unwrap();
        let connection = database.connection();
        let project_id = ProjectId(i64::from(i32::MAX) + 1);
        let event_id = EventId(project_id.0 + 100);
        connection
            .execute(
                "INSERT INTO project (id, name) VALUES (?1, 'Large ID')",
                [project_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO event (id, project_id, event_type, timestamp) VALUES (?1, ?2, 0, 0)",
                rusqlite::params![event_id, project_id],
            )
            .unwrap();

        let projects = ProjectRepository::new(connection);
        assert_eq!(projects.get(project_id).unwrap().unwrap().id, project_id);
        let next_id = projects.insert("Next project", None).unwrap();
        assert_eq!(next_id, ProjectId(project_id.0 + 1));
        assert_eq!(projects.get(next_id).unwrap().unwrap().id, next_id);

        let events = EventRepository::new(connection);
        let event = events.get(event_id).unwrap().unwrap();
        assert_eq!(event.id, event_id);
        assert_eq!(event.project_id, project_id);
        assert!(events.delete(event_id).unwrap());
        assert!(events.get(event_id).unwrap().is_none());
    }

    #[test]
    fn cli_parses_i64_project_ids_and_rejects_invalid_values() {
        let cli = Cli::try_parse_from(["tlog", "start", "--project", "2147483648"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Start {
                project_id: ProjectId(2_147_483_648)
            })
        ));
        for invalid in ["abc", "9223372036854775808"] {
            assert!(Cli::try_parse_from(["tlog", "start", "--project", invalid]).is_err());
        }
    }

    #[test]
    fn id_display_preserves_numeric_formatting() {
        assert_eq!(format!("{:>4}", ProjectId(7)), "   7");
        assert_eq!(format!("{:04}", EventId(7)), "0007");
    }
}
