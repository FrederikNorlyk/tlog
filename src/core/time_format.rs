use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, ValueEnum, Debug)]
pub enum TimeFormat {
    HoursMinutesSeconds,
    HoursMinutes,
    DecimalHours,
    Seconds,
}

impl TimeFormat {
    #[must_use]
    pub fn get_next_format(self) -> TimeFormat {
        match self {
            TimeFormat::HoursMinutesSeconds => TimeFormat::HoursMinutes,
            TimeFormat::HoursMinutes => TimeFormat::DecimalHours,
            TimeFormat::DecimalHours => TimeFormat::Seconds,
            TimeFormat::Seconds => TimeFormat::HoursMinutesSeconds,
        }
    }

    #[must_use]
    pub fn round(self, seconds: i64) -> i64 {
        match self {
            TimeFormat::HoursMinutesSeconds | TimeFormat::Seconds => seconds,
            TimeFormat::HoursMinutes => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
                let mut minutes = (seconds as f64 / 60.0).round() as i64;

                // if non-zero but rounded below 1 minute, round up to 1 minute
                if seconds > 0 && minutes < 1 {
                    minutes = 1;
                }

                minutes * 60
            }
            TimeFormat::DecimalHours => {
                #[allow(clippy::cast_precision_loss)]
                let hours = seconds as f64 / 3600.0;

                // round to nearest quarter-hour
                let mut rounded_hours = (hours * 4.0).round() / 4.0;

                // if non-zero but below 0.25, round up to 0.25
                if hours > 0.0 && rounded_hours < 0.25 {
                    rounded_hours = 0.25;
                }

                #[allow(clippy::cast_possible_truncation)]
                let result = (rounded_hours * 3600.0) as i64;

                result
            }
        }
    }

    #[must_use]
    pub fn format(self, seconds: i64) -> String {
        let seconds = self.round(seconds);

        match self {
            TimeFormat::HoursMinutesSeconds => {
                let (h, m, s) = helpers::seconds_to_hms(seconds);
                format!("{h:02}:{m:02}:{s:02}")
            }
            TimeFormat::HoursMinutes => {
                let (h, m, _) = helpers::seconds_to_hms(seconds);
                format!("{h:02}:{m:02}")
            }
            TimeFormat::DecimalHours => {
                #[allow(clippy::cast_precision_loss)]
                let hours = seconds as f64 / 3600.0;
                format!("{hours:05.2}")
            }
            TimeFormat::Seconds => seconds.to_string(),
        }
    }

    /// Takes a user's input in the form of a string and attempts to convert it to a number of seconds.
    ///
    /// # Errors
    /// Returns an error if the supplied input is invalid
    pub fn parse(self, text: &str) -> Result<i64, String> {
        if text.is_empty() {
            return Err("Value cannot be empty".to_string());
        }

        match self {
            TimeFormat::Seconds => {
                let seconds: i64 = text
                    .parse()
                    .map_err(|_| "Expected whole seconds (e.g. 120)".to_string())?;

                if seconds < 0 {
                    return Err("Seconds must be >= 0".to_string());
                }

                Ok(seconds)
            }

            TimeFormat::HoursMinutesSeconds => {
                let parts: Vec<&str> = text.split(':').collect();

                if parts.len() > 3 {
                    return Err(
                        "Expected format HH[:MM[:SS]] (e.g. 1, 01:30, 01:30:15)".to_string()
                    );
                }

                let (h, m, s) = helpers::parse_hms(&parts)?;

                if m >= 60 || s >= 60 {
                    return Err("Minutes and seconds must be < 60".to_string());
                }

                Ok(h * 3600 + m * 60 + s)
            }

            TimeFormat::HoursMinutes => {
                let parts: Vec<&str> = text.split(':').collect();

                if parts.len() > 2 {
                    return Err("Expected format HH[:MM] (e.g. 1, 01:30)".to_string());
                }

                let (h, m, _) = helpers::parse_hms(&parts)?;

                if m >= 60 {
                    return Err("Minutes must be < 60".to_string());
                }

                Ok(h * 3600 + m * 60)
            }

            TimeFormat::DecimalHours => {
                let value = if text.contains(':') {
                    let parts: Vec<&str> = text.split(':').collect();

                    if parts.len() > 2 {
                        return Err("Expected format HH[:MM] (e.g. 1, 1.5, 01:30)".to_string());
                    }

                    let (h, m, _) = helpers::parse_hms(&parts)?;

                    #[allow(clippy::cast_precision_loss)]
                    let m = m as f64;

                    if m >= 60.0 {
                        return Err("Minutes must be < 60".to_string());
                    }

                    #[allow(clippy::cast_precision_loss)]
                    let seconds = h as f64 + (m / 60.0);

                    seconds
                } else {
                    text.parse::<f64>()
                        .map_err(|_| "Expected decimal hours (e.g. 1.5) or HH:MM".to_string())?
                };

                if value < 0.0 {
                    return Err("Value must be >= 0".to_string());
                }

                #[allow(clippy::cast_possible_truncation)]
                let seconds = (value * 3600.0).round() as i64;

                Ok(seconds)
            }
        }
    }
}

mod helpers {
    #[must_use]
    pub(super) fn seconds_to_hms(seconds: i64) -> (i64, i64, i64) {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let seconds = seconds % 60;

        (hours, minutes, seconds)
    }

    pub(super) fn parse_hms(parts: &[&str]) -> Result<(i64, i64, i64), String> {
        let h: i64 = parts
            .first()
            .unwrap_or(&"0")
            .parse()
            .map_err(|_| "Invalid hours".to_string())?;

        let m: i64 = parts
            .get(1)
            .unwrap_or(&"0")
            .parse()
            .map_err(|_| "Invalid minutes".to_string())?;

        let s: i64 = parts
            .get(2)
            .unwrap_or(&"0")
            .parse()
            .map_err(|_| "Invalid seconds".to_string())?;

        Ok((h, m, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod seconds_to_hms {
        use super::*;

        #[test]
        fn zero() {
            assert_eq!(helpers::seconds_to_hms(0), (0, 0, 0));
        }

        #[test]
        fn normal() {
            // 3661 seconds = 1 hour, 1 minute, 1 second
            assert_eq!(helpers::seconds_to_hms(3661), (1, 1, 1));
        }
    }

    mod round_seconds {
        use super::*;

        #[test]
        fn hms_no_change() {
            assert_eq!(TimeFormat::HoursMinutesSeconds.round(3661), 3661);
        }

        #[test]
        fn seconds_no_change() {
            assert_eq!(TimeFormat::Seconds.round(3661), 3661);
        }

        mod hours_minutes {
            use super::*;

            #[test]
            fn rounds_to_nearest_minute() {
                // 89 sec → 1.48 min → 1 min
                assert_eq!(TimeFormat::HoursMinutes.round(89), 60);

                // 91 sec → 1.52 min → 2 min
                assert_eq!(TimeFormat::HoursMinutes.round(91), 120);
            }

            #[test]
            fn rounds_small_non_zero_up() {
                // Non-zero durations never become 00:00
                assert_eq!(TimeFormat::HoursMinutes.round(1), 60);

                assert_eq!(TimeFormat::HoursMinutes.round(29), 60);
            }
        }

        mod decimal_hours {
            use super::*;

            #[test]
            fn rounds_to_quarters() {
                // Exact hour stays unchanged
                assert_eq!(TimeFormat::DecimalHours.round(3600), 3600);

                // 4500 sec = 1.25h exact quarter
                assert_eq!(TimeFormat::DecimalHours.round(4500), 4500);

                // rounding up
                // 5000 sec ≈ 1.39h → 1.50h
                assert_eq!(TimeFormat::DecimalHours.round(5000), 5400);

                // rounding down
                // 3900 sec = 1.083h → 1.00h
                assert_eq!(TimeFormat::DecimalHours.round(3900), 3600);
            }

            #[test]
            fn rounds_small_non_zero_up() {
                // Never show 0.00 hours for non-zero input
                assert_eq!(TimeFormat::DecimalHours.round(1), 900);

                assert_eq!(TimeFormat::DecimalHours.round(300), 900);
            }
        }
    }

    mod seconds_to_duration {
        use super::*;

        mod hours_minutes {
            use super::*;

            #[test]
            fn formats() {
                // 3661 sec → 3660 sec → 01:01
                assert_eq!(TimeFormat::HoursMinutes.format(3661), "01:01");

                // Small non-zero → normalized to 1 minute
                assert_eq!(TimeFormat::HoursMinutes.format(1), "00:01");
            }
        }

        mod decimal_hours {
            use super::*;

            #[test]
            fn formats() {
                // 300 sec → 5 min → 0.083h → normalized to minimum 0.25h
                assert_eq!(TimeFormat::DecimalHours.format(300), "00.25");

                // 5000 sec → 83 min 20 sec → 1.388h → rounded to nearest quarter (1.50h)
                assert_eq!(TimeFormat::DecimalHours.format(5000), "01.50");

                assert_eq!(TimeFormat::DecimalHours.format(3600), "01.00");
            }
        }

        #[test]
        fn seconds_formats() {
            assert_eq!(TimeFormat::Seconds.format(3661), "3661");
        }

        #[test]
        fn hours_minutes_seconds_formats() {
            assert_eq!(TimeFormat::HoursMinutesSeconds.format(3661), "01:01:01");
        }
    }

    mod string_to_seconds {
        use super::*;

        fn assert_invalid_input(text: &str, expected_error: &str, time_format: TimeFormat) {
            let error = time_format.parse(text).unwrap_err();
            assert_eq!(expected_error, error);
        }

        fn assert_valid_input(text: &str, time_format: TimeFormat) {
            let result = time_format.parse(text);
            assert!(result.is_ok());
        }

        #[test]
        fn empty_text() {
            assert_invalid_input("", "Value cannot be empty", TimeFormat::HoursMinutesSeconds);
        }

        mod seconds {
            use super::*;

            #[test]
            fn valid_input() {
                assert_valid_input("1", TimeFormat::Seconds);
                assert_valid_input("123", TimeFormat::Seconds);
            }

            #[test]
            fn negative() {
                assert_invalid_input("-1", "Seconds must be >= 0", TimeFormat::Seconds);
            }
        }

        mod hours_minutes_seconds {
            use super::*;

            #[test]
            fn valid_input() {
                assert_valid_input("1", TimeFormat::HoursMinutesSeconds);
                assert_valid_input("1:30", TimeFormat::HoursMinutesSeconds);
                assert_valid_input("01:30", TimeFormat::HoursMinutesSeconds);
                assert_valid_input("1:45:33", TimeFormat::HoursMinutesSeconds);
                assert_valid_input("01:45:33", TimeFormat::HoursMinutesSeconds);
            }

            #[test]
            fn too_many_parts() {
                assert_invalid_input(
                    "01:30:15:00",
                    "Expected format HH[:MM[:SS]] (e.g. 1, 01:30, 01:30:15)",
                    TimeFormat::HoursMinutesSeconds,
                );
            }

            #[test]
            fn invalid_hours() {
                assert_invalid_input("aa:30:15", "Invalid hours", TimeFormat::HoursMinutesSeconds);
                assert_invalid_input("x", "Invalid hours", TimeFormat::HoursMinutesSeconds);
            }

            #[test]
            fn invalid_minutes() {
                assert_invalid_input(
                    "01:aa:15",
                    "Invalid minutes",
                    TimeFormat::HoursMinutesSeconds,
                );
            }

            #[test]
            fn invalid_seconds() {
                assert_invalid_input(
                    "01:30:aa",
                    "Invalid seconds",
                    TimeFormat::HoursMinutesSeconds,
                );
            }

            #[test]
            fn out_of_range() {
                assert_invalid_input(
                    "01:90:00",
                    "Minutes and seconds must be < 60",
                    TimeFormat::HoursMinutesSeconds,
                );
                assert_invalid_input(
                    "01:50:80",
                    "Minutes and seconds must be < 60",
                    TimeFormat::HoursMinutesSeconds,
                );
            }
        }

        mod hours_minutes {
            use super::*;

            #[test]
            fn valid_input() {
                assert_valid_input("1", TimeFormat::HoursMinutes);
                assert_valid_input("1:30", TimeFormat::HoursMinutes);
                assert_valid_input("01:30", TimeFormat::HoursMinutes);
            }

            #[test]
            fn too_many_parts() {
                assert_invalid_input(
                    "01:30:15",
                    "Expected format HH[:MM] (e.g. 1, 01:30)",
                    TimeFormat::HoursMinutes,
                );
            }

            #[test]
            fn invalid_hours() {
                assert_invalid_input("aa:30", "Invalid hours", TimeFormat::HoursMinutes);
            }

            #[test]
            fn invalid_minutes() {
                assert_invalid_input("01:aa", "Invalid minutes", TimeFormat::HoursMinutes);
            }

            #[test]
            fn minutes_out_of_range() {
                assert_invalid_input("01:60", "Minutes must be < 60", TimeFormat::HoursMinutes);
            }
        }

        mod decimal_hours {
            use super::*;

            #[test]
            fn valid_input() {
                assert_valid_input("1", TimeFormat::DecimalHours);
                assert_valid_input("1.5", TimeFormat::DecimalHours);
                assert_valid_input("01.50", TimeFormat::DecimalHours);
                assert_valid_input("01.50", TimeFormat::DecimalHours);
                assert_valid_input("1:30", TimeFormat::DecimalHours);
            }

            #[test]
            fn too_many_parts() {
                assert_invalid_input(
                    "01:30:15",
                    "Expected format HH[:MM] (e.g. 1, 1.5, 01:30)",
                    TimeFormat::DecimalHours,
                );
            }

            #[test]
            fn invalid_hours() {
                assert_invalid_input("aa:30", "Invalid hours", TimeFormat::DecimalHours);
            }

            #[test]
            fn invalid_minutes() {
                assert_invalid_input("01:aa", "Invalid minutes", TimeFormat::DecimalHours);
            }

            #[test]
            fn minutes_out_of_range() {
                assert_invalid_input("01:60", "Minutes must be < 60", TimeFormat::DecimalHours);
            }

            #[test]
            fn invalid_decimal() {
                assert_invalid_input(
                    "x",
                    "Expected decimal hours (e.g. 1.5) or HH:MM",
                    TimeFormat::DecimalHours,
                );
            }

            #[test]
            fn negative() {
                assert_invalid_input("-120", "Value must be >= 0", TimeFormat::DecimalHours);
            }
        }
    }
}
