use crate::model::project::Project;

pub struct Session {
    pub project: Project,
    pub total_seconds: i64,
    pub is_started: bool,
}
