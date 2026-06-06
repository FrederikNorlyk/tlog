use time::Date;

pub struct ManualSession {
    pub project_id: i32,
    pub date: Date,
    pub total_seconds: i64,
}
