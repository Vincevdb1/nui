use walkdir::WalkDir;
use crate::context::NixFile;

pub fn find_nix_files() -> Vec<NixFile> {
    let mut nix_files = Vec::new();
    let current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return Vec::new(),
    };

    for entry in WalkDir::new(&current_dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            name != ".git" && name != "target" && name != "node_modules" && name != "result"
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            name == "flake.nix" // || name == "shell.nix" || name == "default.nix"
        })
    {
        if let Ok(relative_path) = entry.path().strip_prefix(&current_dir) {
            nix_files.push(NixFile::new(relative_path.to_path_buf()));
        }
    }

    nix_files
}
