use directories::ProjectDirs;

pub struct Paths;

impl Paths {
    #[must_use]
    pub fn project_dir() -> Option<ProjectDirs> {
        ProjectDirs::from("com", "FrederikNorlyk", "tlog")
    }
}
