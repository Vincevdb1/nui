use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SKIPPED_DIRS: [&str; 4] = [".git", "target", "node_modules", "result"];

/// `flake.nix` files up to three levels below the working directory, as relative paths.
pub fn find() -> Vec<PathBuf> {
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    WalkDir::new(&cwd)
        .max_depth(3)
        .into_iter()
        .filter_entry(|e| !SKIPPED_DIRS.contains(&e.file_name().to_str().unwrap_or("")))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == "flake.nix")
        .filter_map(|e| e.path().strip_prefix(&cwd).ok().map(Path::to_path_buf))
        .collect()
}

pub fn dir(flake_nix: &Path) -> &Path {
    flake_nix.parent().unwrap_or(Path::new("."))
}

pub fn read_lock(flake_nix: &Path) -> Option<String> {
    std::fs::read_to_string(dir(flake_nix).join("flake.lock")).ok()
}

/// A path nix accepts as a flake reference: bare relative paths need a `./` prefix.
pub fn flake_ref(path: &Path) -> String {
    let path = path.to_string_lossy();
    if path.is_empty() || path == "." {
        ".".to_string()
    } else if path.starts_with('/') || path.starts_with("./") || path.contains(':') {
        path.to_string()
    } else {
        format!("./{}", path)
    }
}
