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
        if let Ok(path) = std::env::var("TLOG_DATA_DIR") {
            return Some(PathBuf::from(path));
        }

        Self::project_dir().map(|project_dir| project_dir.data_dir().to_path_buf())
    }

    #[must_use]
    pub fn config_dir() -> Option<PathBuf> {
        if let Ok(path) = std::env::var("TLOG_CONFIG_DIR") {
            return Some(PathBuf::from(path));
        }

        Self::project_dir().map(|project_dir| project_dir.config_dir().to_path_buf())
    }
}
