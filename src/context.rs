pub mod nix_file;
pub mod scanner;

pub use nix_file::NixFile;
pub use scanner::find_nix_files;

use crate::action::Action;
use std::sync::mpsc::Sender;

pub struct Context {
    pub tx: Sender<Action>,
}

impl Context {
    pub fn new(tx: Sender<Action>) -> Self {
        Self { tx }
    }
}
