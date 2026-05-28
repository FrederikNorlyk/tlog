use crate::db::database::Repository;
use crate::model::project::Project;
use rusqlite::{Connection, OptionalExtension, Result, named_params};

pub struct ProjectRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ProjectRepository<'a> {
    #[must_use]
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Inserts a new project.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the insert statement, for
    /// example because the database connection is invalid, the `project` table
    /// does not exist, or the provided data violates a database constraint.
    pub fn insert(&self, name: &str, description: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO project (name, description) VALUES (:name, :description)",
            named_params! {":name": name, ":description": description},
        )?;

        Ok(())
    }

    /// Updates an existing project.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the update statement, for
    /// example because the database connection is invalid, the `project` table
    /// does not exist, or the provided data violates a database constraint.
    pub fn update(&self, project: &Project) -> Result<()> {
        self.conn.execute(
            "UPDATE project SET name = :name, description = :description WHERE id = :id",
            named_params! {
                ":name": &project.name,
                ":description": project.description.as_deref(),
                ":id": project.id,
            },
        )?;

        Ok(())
    }

    /// Deletes the project with the given ID.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the delete statement, for
    /// example because the database connection is invalid or the `project` table
    /// does not exist.
    pub fn delete(&self, id: u32) -> Result<bool> {
        let deleted_count = self.conn.execute(
            "DELETE FROM project WHERE id = (:id)",
            named_params! {":id": id},
        )?;

        Ok(deleted_count > 0)
    }

    /// Gets the project with the given ID.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the query, if the `project`
    /// table does not exist, if no project exists with the given ID, or if the
    /// returned row cannot be converted into a [`Project`].
    pub fn get(&self, id: u32) -> Result<Option<Project>> {
        self.conn
            .query_row(
                "SELECT id, name, description FROM project WHERE id = :id",
                named_params! {":id": id},
                Self::project_from_row,
            )
            .optional()
    }

    /// Calls the provided function once for each project in the database.
    ///
    /// # Errors
    ///
    /// Returns an error if preparing or executing the query fails, or if a row
    /// cannot be converted into a [`Project`].
    pub fn for_each<F>(&self, mut f: F) -> Result<()>
    where
        F: FnMut(Project),
    {
        let mut stmt = self.conn.prepare("SELECT * FROM project")?;

        let rows = stmt.query_map([], Self::project_from_row)?;

        for project in rows {
            f(project?);
        }

        Ok(())
    }

    fn project_from_row(row: &rusqlite::Row<'_>) -> Result<Project> {
        Ok(Project {
            id: row.get("id")?,
            name: row.get("name")?,
            description: row.get("description")?,
        })
    }
}

impl Repository for ProjectRepository<'_> {
    fn initialize_schema(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS project (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                description TEXT CHECK(description IS NULL OR length(trim(description)) > 0)
            )",
            (),
        )?;

        Ok(())
    }
}
