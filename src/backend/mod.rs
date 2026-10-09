pub mod github;
pub mod nix;
pub mod nix_search;
pub mod nxv;
pub mod process;
pub mod search;
pub mod system;
pub mod templates;

use std::process::ExitStatus;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "Failed to execute {0}: {1}. Make sure it's installed and you have an internet connection."
    )]
    Spawn(&'static str, std::io::Error),
    #[error("{0} failed with status: {1}. This might be due to a connection issue.")]
    Failed(&'static str, ExitStatus),
    #[error("Failed to parse {0} output: {1}")]
    Parse(&'static str, serde_json::Error),
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, Error>;
