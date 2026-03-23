use crate::context::{NixFile, find_nix_files};
use crate::nix::{Input, Configuration, flake::{extract_inputs, extract_configurations}, suggestions::Suggestions};
use std::sync::mpsc::{self, Receiver, Sender};

pub struct App {
    pub should_quit: bool,
    pub selected_index: usize,
    pub nix_files: Vec<NixFile>,
    pub selected_nix_file_index: usize,
    pub selected_configuration_index: usize,
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
            selected_configuration_index: 0,
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
            self.update_suggestions();
            self.suggestions.is_loading = false;
        }
    }

    pub fn update_suggestions(&mut self) {
        let existing_urls: Vec<String> = self.inputs.iter().map(|i| i.url.clone()).collect();
        self.suggestions.update_filtered(&self.new_input_name, &existing_urls);
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

    pub fn update_context(&mut self) {
        if let Some(file) = self.nix_files.get(self.selected_nix_file_index) {
            let content = std::fs::read_to_string(&file.path).unwrap_or_default();
            self.inputs = extract_inputs(&content);
            self.configurations = extract_configurations(&content);
            if self.selected_configuration_index >= self.configurations.len() {
                self.selected_configuration_index = 0;
            }
        }
    }

    pub fn add_input(&mut self) {
        let path = if let Some(file) = self.nix_files.get(self.selected_nix_file_index) {
            file.path.clone()
        } else {
            std::path::PathBuf::from("flake.nix")
        };

        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let new_content = crate::nix::flake::add_input(&content, &self.new_input_name, &self.new_input_url);
        if let Err(e) = std::fs::write(&path, new_content) {
            eprintln!("Failed to write {:?}: {}", path, e);
        }
        
        self.update_context();
        
        self.is_adding_input = false;
        self.new_input_name.clear();
        self.new_input_url.clear();
        self.input_cursor = 0;
    }
}
