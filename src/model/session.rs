use std::fmt;
use std::fmt::Formatter;

pub struct Session {
    pub project_id: i32,
    pub total_seconds: i64,
}

impl fmt::Display for Session {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        const LIGHT_GRAY: &str = "\x1b[90m";
        const BOLD: &str = "\x1b[1m";
        const RESET: &str = "\x1b[0m";

        let project_id = self.project_id;
        let total_seconds = &self.total_seconds;

        write!(
            f,
            "{LIGHT_GRAY}{project_id:>2}{RESET}  {BOLD}{total_seconds}{RESET}"
        )?;

        Ok(())
    }
}
