#[derive(Debug, Clone)]
pub struct Issue {
    pub id: String,
    pub description: String,
}

impl Issue {
    #[must_use]
    pub fn new(id: String, description: String) -> Self {
        Self { id, description }
    }
}
