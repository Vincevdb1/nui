use crate::context::{find_nix_files, NixFile};

pub struct App {
    pub should_quit: bool,
    pub selected_index: usize,
    pub nix_files: Vec<NixFile>,
    pub selected_nix_file_index: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            selected_index: 2,
            nix_files: find_nix_files(),
            selected_nix_file_index: 0,
        }
    }

    pub fn tick(&mut self) {}

    pub fn next_tab(&mut self) {
        self.selected_index = if self.selected_index >= 4 { 1 } else { self.selected_index + 1 };
    }

    pub fn previous_tab(&mut self) {
        self.selected_index = if self.selected_index <= 1 { 4 } else { self.selected_index - 1 };
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}
