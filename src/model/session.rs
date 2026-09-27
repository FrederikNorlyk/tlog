use crate::model::project::Project;
use std::time::Duration;

pub struct Session {
    pub project: Project,
    pub duration: Duration,
    pub is_started: bool,
}
