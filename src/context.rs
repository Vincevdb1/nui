pub mod nix_file;
pub mod scanner;

pub use nix_file::NixFile;
pub use scanner::find_nix_files;

use std::sync::mpsc::Sender;
use crate::action::Action;

pub struct Context {
    pub tx: Sender<Action>,
}

impl Context {
    pub fn new(tx: Sender<Action>) -> Self {
        Self { tx }
    }
}
