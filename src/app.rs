use crate::context::{NixFile, find_nix_files};
use crate::nix::{Input, flake::extract_inputs};

pub struct App {
    pub should_quit: bool,
    pub selected_index: usize,
    pub nix_files: Vec<NixFile>,
    pub selected_nix_file_index: usize,
    pub inputs: Vec<Input>,
    pub is_adding_input: bool,
    pub new_input_name: String,
    pub new_input_url: String,
    pub input_cursor: usize, // 0 for name, 1 for url
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
            is_adding_input: false,
            new_input_name: String::new(),
            new_input_url: String::new(),
            input_cursor: 0,
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

    pub fn add_input(&mut self) {
        let content = std::fs::read_to_string("flake.nix").unwrap_or_default();
        let new_content = crate::nix::flake::add_input(&content, &self.new_input_name, &self.new_input_url);
        if let Err(e) = std::fs::write("flake.nix", new_content) {
            // In a real app we would log this to command_log
            eprintln!("Failed to write flake.nix: {}", e);
        }
        self.inputs = extract_inputs(&std::fs::read_to_string("flake.nix").unwrap_or_default());
        self.is_adding_input = false;
        self.new_input_name.clear();
        self.new_input_url.clear();
        self.input_cursor = 0;
    }
}
