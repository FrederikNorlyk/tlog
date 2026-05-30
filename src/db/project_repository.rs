use crate::db::database::Repository;
use crate::model::project::Project;
use rusqlite::{Connection, OptionalExtension, Result, named_params, Row};

pub struct ProjectRepository<'a> {
    connection: &'a Connection,
}

impl<'a> ProjectRepository<'a> {
    #[must_use]
    pub fn new(connection: &'a Connection) -> Self {
        Self { connection }
    }

    /// Inserts a new project.
    ///
    /// # Errors
    ///
    /// Returns an error if `SQLite` fails to execute the insert statement, for
    /// example because the database connection is invalid, the `project` table
    /// does not exist, or the provided data violates a database constraint.
    pub fn insert(&self, name: &str, description: Option<&str>) -> Result<()> {
        self.connection.execute(
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
        self.connection.execute(
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
    pub fn delete(&self, id: i32) -> Result<bool> {
        let deleted_count = self.connection.execute(
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
    pub fn get(&self, id: i32) -> Result<Option<Project>> {
        self.connection
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
        F: FnMut(Project) -> Result<()>,
    {
        let mut stmt = self.connection.prepare("SELECT * FROM project")?;

        let rows = stmt.query_map([], Self::project_from_row)?;

        for project in rows {
            f(project?)?;
        }

        Ok(())
    }

    fn project_from_row(row: &Row<'_>) -> Result<Project> {
        Ok(Project {
            id: row.get("id")?,
            name: row.get("name")?,
            description: row.get("description")?,
        })
    }
}

impl<'a> Repository<'a> for ProjectRepository<'a> {
    fn initialize_schema(connection: &'a Connection) -> Result<()> {
        connection.execute(
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
