pub struct UnixTimestamp;

impl UnixTimestamp {
    #[must_use]
    pub fn now() -> i64 {
        time::OffsetDateTime::now_utc().unix_timestamp()
    }
}
