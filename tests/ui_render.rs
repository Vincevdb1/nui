mod common;

use common::shell_app;
use nui::app::update::update;
use nui::app::{Action, App};
use nui::backend::search::{ChannelVersion, SearchResult};
use nui::features::search;
use nui::flake::Input;
use ratatui::{Terminal, backend::TestBackend};

/// Renders the app and returns the screen as text, one line per row.
fn screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(160, 40)).expect("terminal");
    terminal
        .draw(|frame| nui::ui::render(app, frame))
        .expect("draw");
    let buffer = terminal.backend().buffer();
    buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn line_with<'a>(screen: &'a str, needle: &str) -> &'a str {
    screen
        .lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line contains {needle:?}:\n{screen}"))
}

fn flake(name: &str) -> Input {
    Input {
        name: name.to_string(),
        url: format!("github:owner/{name}"),
        ..Default::default()
    }
}

fn result(name: &str, channel: &str, source_input: Option<&str>) -> SearchResult {
    SearchResult {
        name: name.to_string(),
        description: String::new(),
        versions: vec![ChannelVersion {
            version: "1.0".to_string(),
            channel: channel.to_string(),
            locked_version: None,
        }],
        platforms: vec![],
        is_unfree: false,
        source_input: source_input.map(str::to_string),
        hash: None,
    }
}

fn searching(mut app: App, results: Vec<SearchResult>) -> App {
    search::open(&mut app);
    let generation = app.search_generation;
    update(&mut app, Action::SearchFinished(generation, Ok(results)));
    app
}

#[test]
fn the_shell_panel_lists_the_added_flakes() {
    let mut app = shell_app();
    assert!(!screen(&mut app).contains("Flakes:"));

    app.shell.flakes = vec![flake("nixvim"), flake("tools")];
    assert!(screen(&mut app).contains("Flakes: nixvim, tools"));
}

#[test]
fn the_search_popup_lists_what_is_searched() {
    let mut app = searching(shell_app(), vec![]);
    assert!(screen(&mut app).contains("Sources: nixpkgs"));

    app.shell.flakes = vec![flake("nixvim"), flake("tools")];
    assert!(screen(&mut app).contains("Sources: nixpkgs, nixvim, tools"));
}

#[test]
fn shell_search_results_name_their_source() {
    let mut app = shell_app();
    app.shell.flakes = vec![flake("nixvim")];
    let mut app = searching(
        app,
        vec![
            result("vim", "nxv", None),
            result("nixvim.default", "nixvim", Some("nixvim")),
        ],
    );
    let screen = screen(&mut app);
    assert!(line_with(&screen, "Name").contains("Source"));
    assert!(
        line_with(&screen, ">> ").contains("nixpkgs"),
        "the selected nixpkgs hit"
    );
    let hit = line_with(&screen, "default");
    assert!(hit.contains("nixvim"), "the flake is named: {hit}");
    assert!(
        !hit.contains("nixvim.default"),
        "the name is the attribute alone: {hit}"
    );
}

#[test]
fn the_source_column_is_hidden_without_flakes() {
    let mut app = searching(shell_app(), vec![result("vim", "nxv", None)]);
    let screen = screen(&mut app);
    assert!(!line_with(&screen, "Name").contains("Source"));
    assert!(!line_with(&screen, ">> ").contains("nixpkgs"));
}
