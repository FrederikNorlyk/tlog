use directories::ProjectDirs;

pub struct Paths;

impl Paths {
    pub fn project_dir() -> Option<ProjectDirs> {
        ProjectDirs::from("com", "FrederikNorlyk", "tlog")
    }
}