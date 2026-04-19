use ratatui::widgets::ListState;
use std::time::Instant;
use throbber_widgets_tui::ThrobberState;

pub struct UiState {
    pub selected_index: usize,
    pub selected_nix_file_index: usize,
    pub selected_configuration_index: usize,
    pub is_adding_input: bool,
    pub is_adding_package: bool,
    pub is_searching_packages: bool,
    pub is_showing_package_details: bool,
    pub package_search_query: String,
    pub package_search_state: ListState,
    pub command_log_state: ListState,
    pub throbber_state: ThrobberState,
    pub new_input_name: String,
    pub new_input_url: String,
    pub input_cursor: usize,
    pub package_fetch_error: Option<String>,
    pub fetching_package_details: bool,
    pub last_search_query: String,
    pub last_search_time: Instant,
    pub shell_package_list_state: ListState,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected_index: 2,
            selected_nix_file_index: 0,
            selected_configuration_index: 0,
            is_adding_input: false,
            is_adding_package: false,
            is_searching_packages: false,
            is_showing_package_details: false,
            package_search_query: String::new(),
            package_search_state: ListState::default(),
            command_log_state: ListState::default(),
            throbber_state: ThrobberState::default(),
            new_input_name: String::new(),
            new_input_url: String::new(),
            input_cursor: 0,
            package_fetch_error: None,
            fetching_package_details: false,
            last_search_query: String::new(),
            last_search_time: Instant::now(),
            shell_package_list_state: ListState::default(),
        }
    }
}
