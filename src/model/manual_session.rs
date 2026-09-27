use crate::model::ids::ProjectId;
use std::time::Duration;
use time::Date;

pub struct ManualSession {
    pub project_id: ProjectId,
    pub date: Date,
    pub duration: Duration,
}

impl ManualSession {
    pub(crate) fn from_row(row: &rusqlite::Row<'_>, date: Date) -> rusqlite::Result<Self> {
        let index = row.as_ref().column_index("total_seconds")?;
        let seconds: i64 = row.get(index)?;
        let seconds = u64::try_from(seconds).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
        })?;
        Ok(Self {
            project_id: row.get("project_id")?,
            date,
            duration: Duration::from_secs(seconds),
        })
    }
}
