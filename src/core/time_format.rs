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
    pub fn get_next_format(self) -> TimeFormat {
        match self {
            TimeFormat::HoursMinutesSeconds => TimeFormat::HoursMinutes,
            TimeFormat::HoursMinutes => TimeFormat::DecimalHours,
            TimeFormat::DecimalHours => TimeFormat::Seconds,
            TimeFormat::Seconds => TimeFormat::HoursMinutesSeconds
        }
    }
}