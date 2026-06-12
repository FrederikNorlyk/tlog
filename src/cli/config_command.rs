use crate::core::time_format::TimeFormat;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Where,
    /// Set time format (Seconds | HoursMinutes | HoursMinutesSeconds | DecimalHours)
    TimeFormat {
        value: Option<TimeFormat>
    },
}
