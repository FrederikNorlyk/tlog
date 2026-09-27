use crate::model::ids::{EventId, ProjectId};
use rusqlite::ToSql;
use rusqlite::types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef};
use time::OffsetDateTime;

#[derive(Debug)]
pub struct Event {
    pub id: EventId,
    pub project_id: ProjectId,
    pub event_type: EventType,
    pub timestamp: OffsetDateTime,
}

impl Event {
    /// Converts a database row into an `Event`.
    ///
    /// Expects the row to contain columns: `id`, `project_id`, `event_type`, `timestamp`.
    ///
    /// # Errors
    ///
    /// Returns an error if any column is missing or cannot be converted.
    pub fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
        let timestamp_index = row.as_ref().column_index("timestamp")?;
        let timestamp =
            OffsetDateTime::from_unix_timestamp(row.get(timestamp_index)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    timestamp_index,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })?;
        Ok(Event {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            event_type: row.get("event_type")?,
            timestamp,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Start,
    Stop,
}

impl EventType {
    pub const START_CODE: i64 = 0;
    pub const STOP_CODE: i64 = 1;
}

impl ToSql for EventType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match self {
            EventType::Start => Ok(Self::START_CODE.into()),
            EventType::Stop => Ok(Self::STOP_CODE.into()),
        }
    }
}

impl FromSql for EventType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_i64()? {
            Self::START_CODE => Ok(EventType::Start),
            Self::STOP_CODE => Ok(EventType::Stop),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        }
    }
}
