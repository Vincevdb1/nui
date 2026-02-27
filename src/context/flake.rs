use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Flake {
    pub name: String,
    pub path: PathBuf,
}

impl Flake {
    pub fn new(path: PathBuf) -> Self {
        let name = path.to_string_lossy().to_string();
        
        Self { name, path }
    }
}
