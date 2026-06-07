use crate::nix::Input;
use crate::state::Mode;
use crate::state::domain::{SearchResult, VersionInfo};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, ListState, Paragraph},
};

use crate::components::popups::centered_rect;
use std::collections::HashMap;

mod details;
mod search;
mod versions;

pub use details::render_details;
pub use search::render_package_search;
pub use versions::render_version_selection;

pub struct AddPackageProps<'a> {
    pub query: &'a str,
    pub results: &'a [SearchResult],
    pub channels: &'a [String],
    pub is_searching: bool,
    pub error: Option<&'a str>,
    pub list_state: &'a mut ListState,
    pub is_selecting_version: bool,
    pub is_fetching_versions: bool,
    pub versions: &'a [VersionInfo],
    pub version_fetch_error: Option<&'a str>,
    pub version_list_state: &'a mut ListState,
    pub installed_packages: &'a HashMap<String, (String, String, bool, String)>,
    pub is_shell_mode: bool,
    pub inputs: &'a [Input],
    pub selected_package_name: Option<&'a String>,
    pub mode: Mode,
    pub system_nixpkgs_version: Option<&'a String>,
    pub system_nixpkgs_hash: Option<&'a String>,
    pub is_swapping: bool,
}

pub fn render(frame: &mut Frame, props: &mut AddPackageProps) {
    let area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, area);

    let title = if props.is_swapping {
        " [ Change Version ] "
    } else {
        " [ Add Package ] "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let search_input = Paragraph::new(props.query).block(
        Block::default()
            .title(" Search Query ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(search_input, chunks[0]);

    if props.is_selecting_version {
        let current_versions = if let Some(pkg_name) = props.selected_package_name {
            props
                .results
                .iter()
                .find(|res| &res.name == pkg_name)
                .map(|res| res.versions.as_slice())
                .unwrap_or(&[])
        } else {
            &[]
        };

        render_version_selection(
            frame,
            chunks[1],
            props.is_fetching_versions,
            props.version_fetch_error,
            props.versions,
            props.version_list_state,
            props.inputs,
            current_versions,
            props.mode.clone(),
        );
    } else {
        render_package_search(
            frame,
            chunks[1],
            props.is_searching,
            props.error,
            props.results,
            props.channels,
            props.list_state,
            props.installed_packages,
            props.is_shell_mode,
            props.system_nixpkgs_version,
            props.system_nixpkgs_hash,
        );
    }

    let footer_text = if props.is_selecting_version {
        "Enter: Select Version | Esc: Back to Search"
    } else {
        "Type: Search | Enter: Add | M-Enter: Versions | Tab: Details | Esc: Close"
    };
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
