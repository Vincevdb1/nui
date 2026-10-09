mod ast;
mod inputs;
mod packages;
mod pin;

pub use inputs::{add_input, remove_input};
pub use packages::{add_package, remove_packages};
pub use pin::{pin_package, unpin_package};

use crate::flake::model::ShellTarget;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("could not find devShell {}.{} in flake.nix", .0.system, .0.name)]
    ShellNotFound(ShellTarget),
    #[error("could not find source input for {0}")]
    NoSourceInput(String),
    #[error("input {0} has no revision in lock file. Run 'nix flake lock' first.")]
    Unlocked(String),
    #[error("could not find package {0} in flake.nix")]
    PackageNotFound(String),
    #[error("package {0} is not pinned")]
    NotPinned(String),
    #[error("invalid pinned package reference: {0}")]
    InvalidPinnedRef(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Applies a pure edit to a flake file, writing only when the content changed.
pub fn rewrite(
    path: &Path,
    edit: impl FnOnce(&str) -> Result<String, EditError>,
) -> Result<(), EditError> {
    let src = std::fs::read_to_string(path)?;
    let edited = edit(&src)?;
    if edited != src {
        std::fs::write(path, edited)?;
    }
    Ok(())
}
