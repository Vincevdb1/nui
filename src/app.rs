use crate::context::{find_nix_files, NixFile};
use crate::nix::{Input, flake::extract_inputs};

pub struct App {
    pub should_quit: bool,
    pub selected_index: usize,
    pub nix_files: Vec<NixFile>,
    pub selected_nix_file_index: usize,
    pub inputs: Vec<Input>,
}

impl App {
    pub fn new() -> Self {
        let flake_content = std::fs::read_to_string("flake.nix").unwrap_or_default();
        let inputs = extract_inputs(&flake_content);

        Self {
            should_quit: false,
            selected_index: 2,
            nix_files: find_nix_files(),
            selected_nix_file_index: 0,
            inputs,
        }
    }

    pub fn tick(&mut self) {}

    pub fn next_tab(&mut self) {
        self.selected_index = match self.selected_index {
            1 => 2,
            2 => 3,
            3 => 4,
            4 => 1,
            _ => 1,
        };
    }

    pub fn previous_tab(&mut self) {
        self.selected_index = match self.selected_index {
            1 => 4,
            2 => 1,
            3 => 2,
            4 => 3,
            _ => 1,
        };
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}
