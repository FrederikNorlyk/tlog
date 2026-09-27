use crate::model::ids::ProjectId;
use time::Date;

pub struct ManualSession {
    pub project_id: ProjectId,
    pub date: Date,
    pub total_seconds: i64,
}
