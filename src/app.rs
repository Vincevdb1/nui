use crate::context::{NixFile, find_nix_files};
use crate::nix::{Input, Configuration, flake::{extract_inputs, extract_configurations}, suggestions::Suggestions};
use std::sync::mpsc::{self, Receiver, Sender};

pub struct App {
    pub should_quit: bool,
    pub selected_index: usize,
    pub nix_files: Vec<NixFile>,
    pub selected_nix_file_index: usize,
    pub inputs: Vec<Input>,
    pub configurations: Vec<Configuration>,
    pub is_adding_input: bool,
    pub new_input_name: String,
    pub new_input_url: String,
    pub input_cursor: usize, // 0 for common inputs, 1 for name, 2 for url
    pub suggestions: Suggestions,
    pub tx: Sender<Vec<(String, String)>>,
    pub rx: Receiver<Vec<(String, String)>>,
}

impl App {
    pub fn new() -> Self {
        let flake_content = std::fs::read_to_string("flake.nix").unwrap_or_default();
        let inputs = extract_inputs(&flake_content);
        let configurations = extract_configurations(&flake_content);
        let (tx, rx) = mpsc::channel();

        Self {
            should_quit: false,
            selected_index: 2,
            nix_files: find_nix_files(),
            selected_nix_file_index: 0,
            inputs,
            configurations,
            is_adding_input: false,
            new_input_name: String::new(),
            new_input_url: String::new(),
            input_cursor: 0,
            suggestions: Suggestions::default(),
            tx,
            rx,
        }
    }

    pub fn process_suggestions(&mut self) {
        if let Ok(branches) = self.rx.try_recv() {
            self.suggestions.all = branches;
            self.suggestions.update_filtered(&self.new_input_name);
            self.suggestions.is_loading = false;
        }
    }

    pub fn start_fetching(&mut self) {
        self.suggestions.is_loading = true;
        Suggestions::fetch_branches(self.tx.clone());
    }

    pub fn tick(&mut self) {
        self.process_suggestions();
    }

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
        let updated_content = std::fs::read_to_string("flake.nix").unwrap_or_default();
        self.inputs = extract_inputs(&updated_content);
        self.configurations = extract_configurations(&updated_content);
        self.is_adding_input = false;
        self.new_input_name.clear();
        self.new_input_url.clear();
        self.input_cursor = 0;
    }
}
