use crate::db::database::Repository;
use crate::model::project::Project;
use rusqlite::{Connection, OptionalExtension, Result, Row, named_params};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::database::Database;

    struct TestContext {
        database: Database,
    }

    impl TestContext {
        fn new() -> Result<Self> {
            let database = Database::new_in_memory_db()?;
            database.init().expect("Failed to initialize database");

            Ok(Self { database })
        }

        fn project_repository(&self) -> ProjectRepository<'_> {
            ProjectRepository::new(self.database.connection())
        }
    }

    #[test]
    fn test_insert() -> Result<()> {
        let context = TestContext::new()?;
        let project_repository = context.project_repository();

        project_repository.insert("Just a name", None)?;
        project_repository.insert("Has a desc", Some("Desc here"))?;

        let project_1 = project_repository
            .get(1)?
            .expect("Should have found project 1");

        assert_eq!(project_1.id, 1);
        assert_eq!(project_1.name, "Just a name");
        assert_eq!(project_1.description, None);

        let project_2 = project_repository
            .get(2)?
            .expect("Should have found project 2");

        assert_eq!(project_2.id, 2);
        assert_eq!(project_2.name, "Has a desc");

        assert_eq!(
            project_2
                .description
                .expect("Project 2 should have a description"),
            "Desc here"
        );

        Ok(())
    }

    #[test]
    fn test_insert_invalid_values_fails() -> Result<()> {
        let context = TestContext::new()?;
        let project_repository = context.project_repository();

        assert!(project_repository.insert("", None).is_err());
        assert!(project_repository.insert("   ", None).is_err());

        assert!(project_repository.insert("Valid name", Some("")).is_err());

        assert!(
            project_repository
                .insert("Valid name", Some("   "))
                .is_err()
        );

        Ok(())
    }

    #[test]
    fn test_update() -> Result<()> {
        let context = TestContext::new()?;
        let project_repository = context.project_repository();

        project_repository.insert("Original", Some("Original desc"))?;

        let mut project = project_repository.get(1)?.expect("Project should exist");

        project.name = "Updated".to_string();
        project.description = Some("Updated desc".to_string());

        project_repository.update(&project)?;

        let updated = project_repository
            .get(1)?
            .expect("Project should still exist");

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.description.as_deref(), Some("Updated desc"));

        let mut project = updated;
        project.description = None;

        project_repository.update(&project)?;

        let updated = project_repository
            .get(1)?
            .expect("Project should still exist");

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.description, None);

        Ok(())
    }

    #[test]
    fn test_delete_existing_project() -> Result<()> {
        let context = TestContext::new()?;
        let project_repository = context.project_repository();

        project_repository.insert("Project", None)?;

        assert!(project_repository.get(1)?.is_some());

        assert!(project_repository.delete(1)?);

        assert!(project_repository.get(1)?.is_none());

        Ok(())
    }

    #[test]
    fn test_delete_non_existing_project() -> Result<()> {
        let context = TestContext::new()?;
        let project_repository = context.project_repository();

        assert!(!project_repository.delete(999)?);

        Ok(())
    }

    #[test]
    fn test_get_non_existing_project() -> Result<()> {
        let context = TestContext::new()?;
        let project_repository = context.project_repository();

        assert!(project_repository.get(999)?.is_none());

        Ok(())
    }

    #[test]
    fn test_for_each() -> Result<()> {
        let context = TestContext::new()?;
        let project_repository = context.project_repository();

        project_repository.insert("Project A", None)?;
        project_repository.insert("Project B", Some("Desc"))?;
        project_repository.insert("Project C", None)?;

        let mut projects = Vec::new();

        project_repository.for_each(|project| {
            projects.push(project);
            Ok(())
        })?;

        assert_eq!(projects.len(), 3);

        assert_eq!(projects[0].name, "Project A");
        assert_eq!(projects[1].name, "Project B");
        assert_eq!(projects[2].name, "Project C");

        Ok(())
    }
}
