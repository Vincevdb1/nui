use walkdir::WalkDir;
use crate::context::Flake;

pub fn find_flakes() -> Vec<Flake> {
    let mut flakes = Vec::new();
    let current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return Vec::new(),
    };

    for entry in WalkDir::new(&current_dir)
        .into_iter()
        // TODO: Make this a config option
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            name != ".git" && name != "target" && name != "node_modules" && name != "result"
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == "flake.nix")
    {
        if let Ok(relative_path) = entry.path().strip_prefix(&current_dir) {
            flakes.push(Flake::new(relative_path.to_path_buf()));
        }
    }

    flakes
}
