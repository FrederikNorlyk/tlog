use std::process::{Command, Output};
use tempfile::TempDir;

struct CliTest {
    directory: TempDir,
}

impl CliTest {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tlog"));
        command
            .env("TLOG_DATA_DIR", self.directory.path().join("data"))
            .env("TLOG_CONFIG_DIR", self.directory.path().join("config"))
            .env_remove("RUST_BACKTRACE")
            .env_remove("RUST_LIB_BACKTRACE");
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
}

#[test]
fn tracking_failure_reports_operation_and_cause_on_stderr() {
    let output = CliTest::new().run(&["stop", "--project", "42"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("stop tracking project 42"), "{error}");
    assert_eq!(
        error
            .matches("no active start event for project 42")
            .count(),
        1,
        "{error}"
    );
    assert!(!error.contains("NoActiveStartEvent"), "{error}");
    assert!(!error.contains("backtrace"), "{error}");
}

#[test]
fn invalid_configuration_reports_path_and_parse_cause() {
    let cli = CliTest::new();
    let config = cli.directory.path().join("config/tlog.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "time_format = [").unwrap();
    let output = cli.run(&["list"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains(config.to_str().unwrap()), "{error}");
    assert_eq!(error.matches("TOML parse error").count(), 1, "{error}");
    assert!(error.contains("Could not load configuration"), "{error}");
}

#[test]
fn database_directory_failure_reports_path_and_os_cause() {
    let cli = CliTest::new();
    let path = cli.directory.path().join("data");
    std::fs::write(&path, "not a directory").unwrap();
    let output = cli.run(&["list"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains(path.to_str().unwrap()), "{error}");
    assert!(error.contains("create database directory"), "{error}");
    assert!(error.contains("Caused by:"), "{error}");
    assert!(error.contains("os error"), "{error}");
}

#[test]
fn duplicate_project_reports_operation_and_sqlite_cause_once() {
    let cli = CliTest::new();
    assert!(cli.run(&["project", "add", "Example"]).status.success());
    let output = cli.run(&["project", "add", "Example"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("create project Example"), "{error}");
    assert_eq!(
        error.matches("UNIQUE constraint failed").count(),
        1,
        "{error}"
    );
    assert!(!error.contains("SqliteFailure"), "{error}");
}

#[test]
fn backtraces_are_opt_in_and_respect_library_override() {
    let cli = CliTest::new();
    let output = cli
        .command()
        .args(["stop", "-p", "42"])
        .env("RUST_BACKTRACE", "1")
        .output()
        .unwrap();
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("Stack backtrace:"), "{error}");
    let output = cli
        .command()
        .args(["stop", "-p", "42"])
        .env("RUST_BACKTRACE", "1")
        .env("RUST_LIB_BACKTRACE", "0")
        .output()
        .unwrap();
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(!error.contains("backtrace"), "{error}");
}

#[test]
fn successful_commands_preserve_stdout_and_leave_stderr_empty() {
    let cli = CliTest::new();
    let output = cli.run(&["project", "add", "Example"]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Project #1 created\n");
    assert!(output.stderr.is_empty());
    let output = cli.run(&["project", "list", "--debug"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Project { id: 1, name: \"Example\", description: None }\n"
    );
    assert!(output.stderr.is_empty());
    let output = cli.run(&["set", "-p", "1", "-d", "2026-09-01", "--duration", "1:30"]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let output = cli.run(&["list", "-d", "2026-09-01"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "\x1b[1m01:30:00  \x1b[0m  \x1b[90m\x1b[90m 1\x1b[0m  \x1b[1mExample\x1b[0m\x1b[0m\n\x1b[1m01:30:00        Total\x1b[0m\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn argument_errors_keep_clap_exit_behavior_and_validation() {
    let cli = CliTest::new();
    for (duration, message) in [
        ("1", "expected hh:mm format"),
        ("x:00", "invalid hours"),
        ("1:x", "invalid minutes"),
        ("1:60", "minutes must be < 60"),
    ] {
        let output = cli.run(&["set", "-p", "1", "--duration", duration]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains(message), "{error}");
    }
    let help = cli.run(&["--help"]);
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    assert!(String::from_utf8(help.stdout).unwrap().contains("Usage:"));
}

#[test]
fn corrupt_database_reports_initialization_path_and_cause() {
    let cli = CliTest::new();
    let path = cli.directory.path().join("data/tlog.sqlite3");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not a sqlite database").unwrap();
    let output = cli.run(&["list"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains(path.to_str().unwrap()), "{error}");
    assert_eq!(
        error.matches("file is not a database").count(),
        1,
        "{error}"
    );
}
