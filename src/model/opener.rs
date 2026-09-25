use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Serialize, Deserialize, Debug)]
pub struct Opener {
    #[serde(rename = "url")]
    url_template: String,
    #[serde(rename = "desc")]
    description: String,
}

impl Opener {
    #[must_use]
    pub fn new(url_template: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            url_template: url_template.into(),
            description: description.into(),
        }
    }

    #[must_use]
    pub fn url_template(&self) -> &str {
        &self.url_template
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn build_url(&self, replacement: &str) -> String {
        self.url_template.replace("%s", replacement)
    }
}

impl Display for Opener {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let url = self.url_template.as_str();
        let desc = self.description.as_str();
        write!(f, "Description: {desc}\nURL: {url}")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn build_url() {
        let opener = Opener::new("https://www.website.com/%s", "Open website");
        let url = opener.build_url("some query");

        assert_eq!("https://www.website.com/some query", url);
    }
}
