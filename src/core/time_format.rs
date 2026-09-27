use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Serialize, Deserialize, Copy, Clone, ValueEnum, Debug, Eq, PartialEq)]
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
    pub fn round(self, duration: Duration) -> Duration {
        let seconds = duration.as_secs();
        let interval = match self {
            TimeFormat::HoursMinutesSeconds | TimeFormat::Seconds => {
                return Duration::from_secs(seconds);
            }
            TimeFormat::HoursMinutes => 60,
            TimeFormat::DecimalHours => 900,
        };
        let remainder = seconds % interval;
        let base = seconds - remainder;
        let rounded = if remainder >= interval / 2 {
            base.saturating_add(interval)
        } else {
            base
        };
        Duration::from_secs(if seconds > 0 {
            rounded.max(interval)
        } else {
            0
        })
    }

    #[must_use]
    pub fn format(self, duration: Duration) -> String {
        let seconds = self.round(duration).as_secs();

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

    /// Takes a user's input in the form of a string and attempts to convert it to a duration with whole-second precision.
    ///
    /// # Errors
    /// Returns an error if the supplied input is invalid
    pub fn parse(self, text: &str) -> Result<Duration, TimeParseError> {
        if text.is_empty() {
            return Err(TimeParseError::Empty);
        }

        match self {
            TimeFormat::Seconds => {
                if text.starts_with('-') {
                    return Err(TimeParseError::NegativeSeconds);
                }
                let seconds: u64 = text
                    .parse()
                    .map_err(|source| TimeParseError::Seconds { source })?;

                Ok(Duration::from_secs(seconds))
            }

            TimeFormat::HoursMinutesSeconds => {
                let parts: Vec<&str> = text.split(':').collect();

                if parts.len() > 3 {
                    return Err(TimeParseError::ExpectedHms);
                }

                let (h, m, s) = helpers::parse_hms(&parts)?;

                if m >= 60 || s >= 60 {
                    return Err(TimeParseError::MinutesSecondsRange);
                }

                helpers::duration_from_hms(h, m, s)
            }

            TimeFormat::HoursMinutes => {
                let parts: Vec<&str> = text.split(':').collect();

                if parts.len() > 2 {
                    return Err(TimeParseError::ExpectedHm);
                }

                let (h, m, _) = helpers::parse_hms(&parts)?;

                if m >= 60 {
                    return Err(TimeParseError::MinutesRange);
                }

                helpers::duration_from_hms(h, m, 0)
            }

            TimeFormat::DecimalHours => {
                let value = if text.contains(':') {
                    let parts: Vec<&str> = text.split(':').collect();

                    if parts.len() > 2 {
                        return Err(TimeParseError::ExpectedDecimalHm);
                    }

                    let (h, m, _) = helpers::parse_hms(&parts)?;

                    #[allow(clippy::cast_precision_loss)]
                    let m = m as f64;

                    if m >= 60.0 {
                        return Err(TimeParseError::MinutesRange);
                    }

                    #[allow(clippy::cast_precision_loss)]
                    let seconds = h as f64 + (m / 60.0);

                    seconds
                } else {
                    text.parse::<f64>()
                        .map_err(|source| TimeParseError::Decimal { source })?
                };

                if value < 0.0 {
                    return Err(TimeParseError::NegativeValue);
                }

                Duration::try_from_secs_f64((value * 3600.0).round())
                    .map_err(|_| TimeParseError::OutOfRange)
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum TimeParseError {
    #[error("Duration is not finite or is too large")]
    OutOfRange,
    #[error("Value cannot be empty")]
    Empty,
    #[error("Expected whole seconds (e.g. 120)")]
    Seconds {
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("Seconds must be >= 0")]
    NegativeSeconds,
    #[error("Expected format HH[:MM[:SS]] (e.g. 1, 01:30, 01:30:15)")]
    ExpectedHms,
    #[error("Expected format HH[:MM] (e.g. 1, 01:30)")]
    ExpectedHm,
    #[error("Expected format HH[:MM] (e.g. 1, 1.5, 01:30)")]
    ExpectedDecimalHm,
    #[error("Minutes and seconds must be < 60")]
    MinutesSecondsRange,
    #[error("Minutes must be < 60")]
    MinutesRange,
    #[error("Expected decimal hours (e.g. 1.5) or HH:MM")]
    Decimal {
        #[source]
        source: std::num::ParseFloatError,
    },
    #[error("Value must be >= 0")]
    NegativeValue,
    #[error("Invalid {component}")]
    Component {
        component: &'static str,
        #[source]
        source: std::num::ParseIntError,
    },
}

mod helpers {
    use super::TimeParseError;
    use std::time::Duration;

    pub(super) fn duration_from_hms(h: u64, m: u64, s: u64) -> Result<Duration, TimeParseError> {
        h.checked_mul(3600)
            .and_then(|hours| {
                m.checked_mul(60)
                    .and_then(|minutes| hours.checked_add(minutes))
            })
            .and_then(|seconds| seconds.checked_add(s))
            .map(Duration::from_secs)
            .ok_or(TimeParseError::OutOfRange)
    }

    #[must_use]
    pub(super) fn seconds_to_hms(seconds: u64) -> (u64, u64, u64) {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let seconds = seconds % 60;

        (hours, minutes, seconds)
    }

    pub(super) fn parse_hms(parts: &[&str]) -> Result<(u64, u64, u64), TimeParseError> {
        if parts.iter().any(|part| part.starts_with('-')) {
            return Err(TimeParseError::NegativeValue);
        }
        let h: u64 =
            parts
                .first()
                .unwrap_or(&"0")
                .parse()
                .map_err(|source| TimeParseError::Component {
                    component: "hours",
                    source,
                })?;

        let m: u64 =
            parts
                .get(1)
                .unwrap_or(&"0")
                .parse()
                .map_err(|source| TimeParseError::Component {
                    component: "minutes",
                    source,
                })?;

        let s: u64 =
            parts
                .get(2)
                .unwrap_or(&"0")
                .parse()
                .map_err(|source| TimeParseError::Component {
                    component: "seconds",
                    source,
                })?;

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
            assert_eq!(
                TimeFormat::HoursMinutesSeconds.round(Duration::from_secs(3661)),
                Duration::from_secs(3661)
            );
        }

        #[test]
        fn seconds_no_change() {
            assert_eq!(
                TimeFormat::Seconds.round(Duration::from_secs(3661)),
                Duration::from_secs(3661)
            );
        }

        mod hours_minutes {
            use super::*;

            #[test]
            fn rounds_to_nearest_minute() {
                // 89 sec → 1.48 min → 1 min
                assert_eq!(
                    TimeFormat::HoursMinutes.round(Duration::from_secs(89)),
                    Duration::from_mins(1)
                );

                // 91 sec → 1.52 min → 2 min
                assert_eq!(
                    TimeFormat::HoursMinutes.round(Duration::from_secs(91)),
                    Duration::from_mins(2)
                );
            }

            #[test]
            fn rounds_small_non_zero_up() {
                // Non-zero durations never become 00:00
                assert_eq!(
                    TimeFormat::HoursMinutes.round(Duration::from_secs(1)),
                    Duration::from_mins(1)
                );

                assert_eq!(
                    TimeFormat::HoursMinutes.round(Duration::from_secs(29)),
                    Duration::from_mins(1)
                );
            }
        }

        mod decimal_hours {
            use super::*;

            #[test]
            fn rounds_to_quarters() {
                // Exact hour stays unchanged
                assert_eq!(
                    TimeFormat::DecimalHours.round(Duration::from_hours(1)),
                    Duration::from_hours(1)
                );

                // 4500 sec = 1.25h exact quarter
                assert_eq!(
                    TimeFormat::DecimalHours.round(Duration::from_mins(75)),
                    Duration::from_mins(75)
                );

                // rounding up
                // 5000 sec ≈ 1.39h → 1.50h
                assert_eq!(
                    TimeFormat::DecimalHours.round(Duration::from_secs(5000)),
                    Duration::from_mins(90)
                );

                // rounding down
                // 3900 sec = 1.083h → 1.00h
                assert_eq!(
                    TimeFormat::DecimalHours.round(Duration::from_mins(65)),
                    Duration::from_hours(1)
                );
            }

            #[test]
            fn rounds_small_non_zero_up() {
                // Never show 0.00 hours for non-zero input
                assert_eq!(
                    TimeFormat::DecimalHours.round(Duration::from_secs(1)),
                    Duration::from_mins(15)
                );

                assert_eq!(
                    TimeFormat::DecimalHours.round(Duration::from_mins(5)),
                    Duration::from_mins(15)
                );
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
                assert_eq!(
                    TimeFormat::HoursMinutes.format(Duration::from_secs(3661)),
                    "01:01"
                );

                // Small non-zero → normalized to 1 minute
                assert_eq!(
                    TimeFormat::HoursMinutes.format(Duration::from_secs(1)),
                    "00:01"
                );
            }
        }

        mod decimal_hours {
            use super::*;

            #[test]
            fn formats() {
                // 300 sec → 5 min → 0.083h → normalized to minimum 0.25h
                assert_eq!(
                    TimeFormat::DecimalHours.format(Duration::from_mins(5)),
                    "00.25"
                );

                // 5000 sec → 83 min 20 sec → 1.388h → rounded to nearest quarter (1.50h)
                assert_eq!(
                    TimeFormat::DecimalHours.format(Duration::from_secs(5000)),
                    "01.50"
                );

                assert_eq!(
                    TimeFormat::DecimalHours.format(Duration::from_hours(1)),
                    "01.00"
                );
            }
        }

        #[test]
        fn seconds_formats() {
            assert_eq!(
                TimeFormat::Seconds.format(Duration::from_secs(3661)),
                "3661"
            );
        }

        #[test]
        fn hours_minutes_seconds_formats() {
            assert_eq!(
                TimeFormat::HoursMinutesSeconds.format(Duration::from_secs(3661)),
                "01:01:01"
            );
        }
    }

    #[test]
    fn invalid_seconds_retains_the_numeric_parse_cause() {
        use std::error::Error;
        let error = TimeFormat::Seconds.parse("abc").unwrap_err();
        assert!(matches!(error, TimeParseError::Seconds { .. }));
        assert!(error.source().unwrap().is::<std::num::ParseIntError>());
    }

    mod string_to_seconds {
        use super::*;

        fn assert_invalid_input(text: &str, expected_error: &str, time_format: TimeFormat) {
            let error = time_format.parse(text).unwrap_err();
            assert_eq!(expected_error, error.to_string());
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
