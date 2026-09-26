use std::io::{self, Write};
use tlog::cli::project_command::{ProjectCommand, handle_project_command};
use tlog::db::{database::Database, project_repository::ProjectRepository};

#[test]
fn project_output_failure_preserves_io_error() {
    struct BrokenOutput;
    impl Write for BrokenOutput {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "reader closed"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let database = Database::new_in_memory_db().unwrap();
    database.init().unwrap();
    let repository = ProjectRepository::new(database.connection());
    repository.insert("Example", None).unwrap();
    let error = handle_project_command(
        ProjectCommand::List { debug: false },
        &repository,
        &mut BrokenOutput,
    )
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<io::Error>().unwrap().kind(),
        io::ErrorKind::BrokenPipe
    );
    let report = tlog::core::diagnostic::report(&error);
    assert!(report.contains("write project to stdout"));
    assert_eq!(report.matches("reader closed").count(), 1);
}

#[test]
fn project_persistence_failure_preserves_sqlite_error() {
    let database = Database::new_in_memory_db().unwrap();
    let repository = ProjectRepository::new(database.connection());
    let error = handle_project_command(
        ProjectCommand::List { debug: false },
        &repository,
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(error.downcast_ref::<rusqlite::Error>().is_some());
    let report = tlog::core::diagnostic::report(&error);
    assert!(report.contains("list projects"));
    assert_eq!(report.matches("no such table: project").count(), 1);
}
