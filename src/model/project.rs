use std::fmt;
use std::fmt::Formatter;

#[derive(Debug)]
pub struct Project {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

impl Project {
    #[must_use]
    pub fn new(id: i32, name: &str, description: Option<&str>) -> Self {
        Self {
            id,
            name: name.to_string(),
            description: description.map(str::to_string),
        }
    }
}

impl fmt::Display for Project {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        const LIGHT_GRAY: &str = "\x1b[90m";
        const BOLD: &str = "\x1b[1m";
        const RESET: &str = "\x1b[0m";

        let id = self.id;
        let name = &self.name;

        write!(f, "{LIGHT_GRAY}{id:>2}{RESET}  {BOLD}{name}{RESET}")?;

        if let Some(description) = &self.description {
            write!(f, "  {LIGHT_GRAY}{description}{RESET}")?;
        }

        Ok(())
    }
}
