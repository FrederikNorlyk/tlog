pub struct FormatUtil;

impl FormatUtil {
    #[must_use]
    pub fn seconds_to_hms(seconds: i64) -> (i64, i64, i64) {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let seconds = seconds % 60;

        (hours, minutes, seconds)
    }

    #[must_use]
    pub fn seconds_to_duration(seconds: i64) -> String {
        let (hours, minutes, seconds) = FormatUtil::seconds_to_hms(seconds);

        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }
}
