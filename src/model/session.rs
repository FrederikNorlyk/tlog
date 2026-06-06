use crate::model::project::Project;
use crate::util::format_util::FormatUtil;
use std::fmt;
use std::fmt::Formatter;

pub struct Session {
    pub project: Project,
    pub total_seconds: i64,
    pub is_started: bool,
}

impl fmt::Display for Session {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        const LIGHT_GRAY: &str = "\x1b[90m";
        const BOLD: &str = "\x1b[1m";
        const RESET: &str = "\x1b[0m";

        let project = &self.project;
        let (hours, minutes, seconds) = FormatUtil::seconds_to_hms(self.total_seconds);

        let running_symbol = if self.is_started { "*" } else { "" };

        write!(
            f,
            "{BOLD}{hours:02}:{minutes:02}:{seconds:02}{running_symbol:>1}{RESET}  {LIGHT_GRAY}{project}{RESET}",
        )
    }
}
