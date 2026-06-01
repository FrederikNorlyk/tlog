use rusqlite::ToSql;
use rusqlite::types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef};

#[derive(Debug)]
pub struct Event {
    pub id: i32,
    pub project_id: i32,
    pub event_type: EventType,
    pub timestamp: i64,
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
