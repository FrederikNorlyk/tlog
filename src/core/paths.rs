use crate::core::constants::{CONFIG_DIR_ENV, DATA_DIR_ENV};
use directories::ProjectDirs;
use std::path::PathBuf;

pub struct Paths;

impl Paths {
    #[must_use]
    fn project_dir() -> Option<ProjectDirs> {
        ProjectDirs::from("com", "FrederikNorlyk", "tlog")
    }

    #[must_use]
    pub fn data_dir() -> Option<PathBuf> {
        if let Ok(path) = std::env::var(DATA_DIR_ENV) {
            return Some(PathBuf::from(path));
        }

        Self::project_dir().map(|project_dir| project_dir.data_dir().to_path_buf())
    }

    #[must_use]
    pub fn config_dir() -> Option<PathBuf> {
        if let Ok(path) = std::env::var(CONFIG_DIR_ENV) {
            return Some(PathBuf::from(path));
        }

        Self::project_dir().map(|project_dir| project_dir.config_dir().to_path_buf())
    }
}

#[allow(unsafe_code)]
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use serial_test::serial;

    #[allow(unsafe_code)]
    fn teardown() {
        unsafe {
            std::env::remove_var(CONFIG_DIR_ENV);
            std::env::remove_var(DATA_DIR_ENV);
        }
    }

    mod data_dir {
        use super::*;

        #[test]
        #[serial]
        fn env_override() {
            let expected = PathBuf::from("/tmp/tlog-data");

            unsafe {
                std::env::set_var(DATA_DIR_ENV, &expected);
            }

            let actual = Paths::data_dir().unwrap();

            assert_eq!(actual, expected);

            teardown();
        }

        #[test]
        #[serial]
        fn default_value() {
            unsafe {
                std::env::remove_var(DATA_DIR_ENV);
            }

            let actual = Paths::data_dir().unwrap();
            let expected = std::env::var("HOME").unwrap();

            assert_eq!(actual, PathBuf::from(expected).join(".local/share/tlog"));

            teardown();
        }
    }

    mod config_dir {
        use super::*;

        #[test]
        #[serial]
        fn env_override() {
            let expected = PathBuf::from("/tmp/tlog-config");

            unsafe {
                std::env::set_var(CONFIG_DIR_ENV, &expected);
            }

            let actual = Paths::config_dir().unwrap();

            assert_eq!(actual, expected);

            teardown();
        }

        #[test]
        #[serial]
        fn default_value() {
            unsafe {
                std::env::remove_var(CONFIG_DIR_ENV);
            }

            let actual = Paths::config_dir().unwrap();
            let expected = std::env::var("HOME").unwrap();

            assert_eq!(actual, PathBuf::from(expected).join(".config/tlog"));

            teardown();
        }
    }
}
