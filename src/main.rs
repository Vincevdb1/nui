use color_eyre::eyre::Result;
use ratatui::crossterm::event::{self, Event, KeyCode};

mod action;
mod app;
mod components;
mod context;
pub mod handlers;
mod nix;
mod state;
mod tui;
mod ui;

use action::Action;
use app::App;

pub use components::command_log::{command_log, log_action, log_output};

fn main() -> Result<()> {
    color_eyre::install()?;
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let (mode, shell_packages) = if args.iter().any(|arg| arg == "shell") {
        (crate::state::Mode::Shell, Vec::new())
    } else {
        (crate::state::Mode::Flake, Vec::new())
    };

    if let Some(config_dir) = dirs::config_dir() {
        let template_dir = config_dir.join("nui").join("templates");
        if !template_dir.exists() {
            let _ = std::fs::create_dir_all(&template_dir);
        }
    }

    let mut terminal = tui::init()?;
    let mut app = App::new(mode.clone(), shell_packages);

    if !std::path::Path::new("flake.nix").exists() && mode == crate::state::Mode::Flake {
        app.ui.show_templates = true;
        if !app.ui.templates.is_empty() {
            app.ui.template_list_state.select(Some(2));
        }
    }

    let result = run(&mut terminal, &mut app);

    tui::restore()?;

    if let Ok(Some(pkgs)) = result {
        if !pkgs.is_empty() {
            let system_nixpkgs_path = app.domain.system_nixpkgs_path.clone();
            let system_version = app.domain.system_nixpkgs_version.clone();
            
            let normalized_pkgs: Vec<String> = pkgs
                .iter()
                .map(|pkg| {
                    let pkg_ref = if let Some((path, _version)) = pkg.rsplit_once('@') {
                        path
                    } else {
                        pkg
                    };

                    if pkg_ref.starts_with("system/") {
                        if let Some((_, suffix)) = pkg_ref.split_once('#') {
                            if let Some(path) = &system_nixpkgs_path {
                                if std::path::Path::new(path).exists() {
                                    return format!("path:{}#{}", path, suffix);
                                }
                            }
                            
                            if let Some(version) = &system_version {
                                let channel = if version.contains("pre") || version.contains("unstable") {
                                    "nixpkgs-unstable"
                                } else {
                                    let parts: Vec<&str> = version.split('.').collect();
                                    if parts.len() >= 2 {
                                        &format!("nixos-{}", &parts[..2].join("."))
                                    } else {
                                        "nixpkgs-unstable"
                                    }
                                };
                                return format!("nixpkgs/{}#{}", channel, suffix);
                            }
                        }
                        
                        let hash_part = &pkg_ref[7..];
                        format!("github:NixOS/nixpkgs/{}", hash_part)
                    } else if pkg_ref.starts_with("nixpkgs/") {
                        let hash_part = &pkg_ref[8..];
                        if hash_part.contains('#') {
                            let (hash, _pkg) = hash_part.split_once('#').unwrap();
                            if hash.len() != 40 && !hash.contains('.') {
                                return format!("nixpkgs#{}", hash_part);
                            }
                        }
                        format!("github:NixOS/nixpkgs/{}", hash_part)
                    } else if pkg_ref.contains('#') {
                        pkg_ref.to_string()
                    } else {
                        format!("nixpkgs#{}", pkg_ref)
                    }
                })
                .collect();

            use color_eyre::owo_colors::OwoColorize;
            println!(
                "Opening shell with packages: {}",
                normalized_pkgs.join(", ").cyan()
            );

            let mut cmd = std::process::Command::new("nix");
            cmd.arg("shell");
            for pkg in normalized_pkgs {
                cmd.arg(pkg);
            }
            cmd.arg("--impure");
            
            let env_name = {
                let shortened_packages: Vec<String> = pkgs.iter().map(|pkg| {
                    let mut display_pkg = pkg.clone();
                    if let Some(hash_idx) = display_pkg.find("nixpkgs/") {
                        if let Some(hash_end) = display_pkg[hash_idx + 8..].find('#') {
                            let hash = &display_pkg[hash_idx + 8..hash_idx + 8 + hash_end];
                            if hash.len() > 7 {
                                display_pkg = format!("{}{}{}", &display_pkg[..hash_idx + 8], &hash[..7], &display_pkg[hash_idx + 8 + hash_end..]);
                            }
                        }
                    } else if let Some(hash_idx) = display_pkg.find("system/") {
                        if let Some(hash_end) = display_pkg[hash_idx + 7..].find('#') {
                            let hash = &display_pkg[hash_idx + 7..hash_idx + 7 + hash_end];
                            let short_hash = if hash.len() > 7 { &hash[..7] } else { hash };
                            display_pkg = format!("{}#{}", short_hash, &display_pkg[hash_idx + 7 + hash_end + 1..]);
                        }
                    }
                    display_pkg
                }).collect();
                format!("nui-shell:{}-env", shortened_packages.join(":"))
            };
            
            cmd.env("name", env_name);
            cmd.env("NIXPKGS_ALLOW_UNFREE", "1");
            cmd.status()?;
        }
    }

    Ok(())
}

fn run(terminal: &mut tui::Tui, app: &mut App) -> Result<Option<Vec<String>>> {
    let mut shell_packages = None;
    while !app.should_quit {
        terminal.draw(|frame| ui::render(app, frame))?;

        if event::poll(std::time::Duration::from_millis(16))?
            && let Some(action) = map_event(app, event::read()?)
        {
            if let Action::StartShell(ref pkgs) = action {
                shell_packages = Some(pkgs.clone());
            }
            app.update(action);
        }
        app.update(Action::Tick);
    }
    Ok(shell_packages)
}

fn map_event(app: &App, event: Event) -> Option<Action> {
    if let Event::Key(key) = event {
        if app.ui.show_help {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => Some(Action::ToggleHelp),
                _ => None,
            };
        }

        if app.ui.show_templates {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('t') => Some(Action::ToggleTemplates),
                KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveTemplateSelectionDown),
                KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveTemplateSelectionUp),
                KeyCode::Enter => {
                    if let Some(i) = app.ui.template_list_state.selected() {
                        if i >= 2 {
                            if let Some((template_name, _)) = app.ui.templates.get(i - 2) {
                                return Some(Action::ApplyTemplate(template_name.clone()));
                            }
                        }
                    }
                    Some(Action::ToggleTemplates)
                }
                _ => None,
            };
        }

        if app.ui.is_saving_shell_template {
            return match key.code {
                KeyCode::Esc => Some(Action::ToggleSaveShellTemplate),
                KeyCode::Tab => Some(Action::NextInputField),
                KeyCode::BackTab => Some(Action::PreviousInputField),
                KeyCode::Char(c) => Some(Action::NewTemplateChar(c)),
                KeyCode::Backspace => Some(Action::NewTemplateBackspace),
                KeyCode::Enter => Some(Action::SaveShellTemplate),
                _ => None,
            };
        }

        if app.ui.is_confirming_template_overwrite {
            return match key.code {
                KeyCode::Char('y') | KeyCode::Enter => Some(Action::ConfirmApplyTemplate),
                KeyCode::Char('n') | KeyCode::Esc => Some(Action::CancelApplyTemplate),
                _ => None,
            };
        }

        if app.ui.is_adding_package {
            if app.ui.is_showing_package_details {
                return match key.code {
                    KeyCode::Esc | KeyCode::Tab | KeyCode::Char('q') => {
                        Some(Action::TogglePackageDetails)
                    }
                    _ => None,
                };
            }

            if app.ui.is_selecting_version {
                return match key.code {
                    KeyCode::Esc => Some(Action::BackToPackageSearch),
                    KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveVersionSelectionDown),
                    KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveVersionSelectionUp),
                    KeyCode::Enter => {
                        if let Some(i) = app.ui.version_list_state.selected() {
                            let mut all_items = Vec::new();

                            if app.mode == crate::state::Mode::Flake {
                                if let Some(pkg_name) = &app.ui.selected_package_name {
                                    let result_opt = app.domain.package_search_results.iter().find(|res| &res.name == pkg_name);
                                    if let Some(result) = result_opt {
                                        for input in &app.domain.inputs {
                                            let channel_name = crate::state::domain::extract_channel(input);
                                            if let Some(cv) = result.versions.iter().find(|v| v.channel == channel_name) {
                                                let version = cv.locked_version.as_ref().unwrap_or(&cv.version).clone();
                                                all_items.push((version, Some(input.name.clone()), None));
                                            }
                                        }
                                    }
                                }
                            }

                            for v in &app.domain.package_versions {
                                all_items.push((v.version.clone(), None, Some(v.clone())));
                            }

                            all_items.sort_by(|a, b| b.0.cmp(&a.0));

                            if let Some(item) = all_items.get(i) {
                                if let Some(input_name) = &item.1 {
                                    return Some(Action::SelectInputForPackage(input_name.clone()));
                                } else if let Some(version_info) = &item.2 {
                                    return Some(Action::SelectVersion(version_info.clone()));
                                }
                            }
                        }
                        None
                    }
                    _ => None,
                };
            }

            return match key.code {
                KeyCode::Esc => Some(Action::ClosePopup),
                KeyCode::Tab => {
                    if !app.domain.package_search_results.is_empty() {
                        Some(Action::TogglePackageDetails)
                    } else {
                        None
                    }
                }
                KeyCode::Char('j') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    Some(Action::MoveSearchSelectionDown)
                }
                KeyCode::Char('k') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    Some(Action::MoveSearchSelectionUp)
                }
                KeyCode::Down => Some(Action::MoveSearchSelectionDown),
                KeyCode::Up => Some(Action::MoveSearchSelectionUp),
                KeyCode::Backspace => Some(Action::PackageSearchBackspace),
                KeyCode::Enter if key.modifiers.contains(event::KeyModifiers::ALT) => {
                    Some(Action::PackageSearchSubmitVersions)
                }
                KeyCode::Enter => Some(Action::PackageSearchSubmitDirect),
                KeyCode::Char(c) => Some(Action::PackageSearchChar(c)),
                _ => None,
            };
        }

        if app.ui.is_adding_input {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') => Some(Action::ClosePopup),
                KeyCode::Tab => Some(Action::NextInputField),
                KeyCode::BackTab => Some(Action::PreviousInputField),
                KeyCode::Down => {
                    if app.ui.input_cursor == 0 {
                        Some(Action::MoveSuggestionDown)
                    } else {
                        Some(Action::NextInputField)
                    }
                }
                KeyCode::Char('j') if app.ui.input_cursor == 0 => {
                    Some(Action::MoveSuggestionDown)
                }
                KeyCode::Up => {
                    if app.ui.input_cursor == 0 {
                        Some(Action::MoveSuggestionUp)
                    } else {
                        Some(Action::PreviousInputField)
                    }
                }
                KeyCode::Char('k') if app.ui.input_cursor == 0 => {
                    Some(Action::MoveSuggestionUp)
                }
                KeyCode::Char(c) => Some(Action::InputPopupChar(c)),
                KeyCode::Backspace => Some(Action::InputPopupBackspace),
                KeyCode::Enter => Some(Action::InputPopupSubmit),
                _ => None,
            };
        }

        if app.ui.is_showing_package_details {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') => Some(Action::TogglePackageDetails),
                _ => None,
            };
        }

        return match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Char('t') => Some(Action::ToggleTemplates),
            KeyCode::Char('T') if app.mode == crate::state::Mode::Shell => Some(Action::ToggleSaveShellTemplate),
            KeyCode::Tab => Some(Action::NextTab),
            KeyCode::BackTab => Some(Action::PreviousTab),
            KeyCode::Char('1') => Some(Action::SelectTab(1)),
            KeyCode::Char('2') => Some(Action::SelectTab(2)),
            KeyCode::Char('3') => Some(Action::SelectTab(3)),
            KeyCode::Char('4') => Some(Action::SelectTab(4)),
            KeyCode::Char('5') => Some(Action::SelectTab(5)),
            KeyCode::Char('a') if app.ui.selected_index == 2 || (app.mode == crate::state::Mode::Shell && app.ui.selected_index == 1) => Some(Action::OpenAddPackage),
            KeyCode::Char('a') if app.ui.selected_index == 3 => Some(Action::OpenAddInput),
            KeyCode::Char('s') if app.mode == crate::state::Mode::Shell && app.ui.selected_index == 1 => Some(Action::StartShell(app.shell_packages.clone())),
            KeyCode::Char(' ') if app.mode == crate::state::Mode::Shell && app.ui.selected_index == 1 => {
                if let Some(i) = app.ui.shell_package_list_state.selected() {
                    if i > 0 {
                        if let Some(pkg_name) = app.shell_packages.get(i - 1) {
                            return Some(Action::ToggleShellPackageSelection(pkg_name.clone()));
                        }
                    }
                }
                None
            }
            KeyCode::Char('d') if app.mode == crate::state::Mode::Shell && app.ui.selected_index == 1 => {
                if !app.ui.selected_shell_packages.is_empty() {
                    let mut indices = Vec::new();
                    for (i, pkg) in app.shell_packages.iter().enumerate() {
                        if app.ui.selected_shell_packages.contains(pkg) {
                            indices.push(i);
                        }
                    }
                    return Some(Action::RemovePackages(indices));
                }
                if let Some(i) = app.ui.shell_package_list_state.selected() {
                    if i > 0 {
                        Some(Action::RemovePackage(i - 1))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            KeyCode::Char(' ') if app.ui.selected_index == 2 => {
                if let Some(i) = app.ui.package_table_state.selected() {
                    if i > 0 {
                        let mut pkgs: Vec<_> = app.domain.package_info.keys().collect();
                        pkgs.sort();
                        if let Some(pkg_name) = pkgs.get(i - 1) {
                            return Some(Action::TogglePackageSelection(pkg_name.to_string()));
                        }
                    }
                }
                None
            }
            KeyCode::Char('d') if app.ui.selected_index == 2 => {
                if !app.ui.selected_packages.is_empty() {
                    let mut pkgs_to_remove = Vec::new();
                    for pkg_name in &app.ui.selected_packages {
                        if let Some((_, _, _, source)) = app.domain.package_info.get(pkg_name) {
                            let full_name = if source.is_empty() {
                                pkg_name.to_string()
                            } else {
                                format!("{}.{}", source, pkg_name)
                            };
                            pkgs_to_remove.push(full_name);
                        }
                    }
                    return Some(Action::RemoveFlakePackages(pkgs_to_remove));
                }

                if let Some(i) = app.ui.package_table_state.selected() {
                    if i > 0 {
                        let mut pkgs: Vec<_> = app.domain.package_info.keys().collect();
                        pkgs.sort();
                        if let Some(pkg_name) = pkgs.get(i - 1) {
                            if let Some((_, _, _, source)) = app.domain.package_info.get(*pkg_name) {
                                let full_name = if source.is_empty() {
                                    pkg_name.to_string()
                                } else {
                                    format!("{}.{}", source, pkg_name)
                                };
                                return Some(Action::RemoveFlakePackage(full_name));
                            }
                        }
                    }
                }
                None
            }
            KeyCode::Char('d') if app.ui.selected_index == 3 => {
                if let Some(i) = app.ui.input_table_state.selected() {
                    if i > 0 && i <= app.domain.inputs.len() {
                        if let Some(input) = app.domain.inputs.get(i - 1) {
                            return Some(Action::RemoveInput(input.name.clone()));
                        }
                    }
                }
                None
            }
            KeyCode::Char('m') => Some(Action::SwitchMode),
            KeyCode::Char('?') => Some(Action::ToggleHelp),
            KeyCode::Char('p') if app.ui.selected_index == 2 || (app.mode == crate::state::Mode::Shell && app.ui.selected_index == 1) => {
                if app.mode == crate::state::Mode::Shell {
                    if let Some(i) = app.ui.shell_package_list_state.selected() {
                        if i > 0 {
                            if let Some(pkg_name) = app.shell_packages.get(i - 1) {
                                return Some(Action::TogglePin(pkg_name.clone()));
                            }
                        }
                    }
                } else {
                    if let Some(i) = app.ui.package_table_state.selected() {
                        if i > 0 {
                            let mut pkgs: Vec<_> = app.domain.package_info.keys().collect();
                            pkgs.sort();
                            if let Some(pkg_name) = pkgs.get(i - 1) {
                                return Some(Action::TogglePin(pkg_name.to_string()));
                            }
                        }
                    }
                }
                None
            }
            KeyCode::Char('i') if app.ui.selected_index == 2 || (app.mode == crate::state::Mode::Shell && app.ui.selected_index == 1) => Some(Action::TogglePackageDetails),
            KeyCode::Char('j') if app.ui.selected_index == 2 => Some(Action::MovePackageSelectionDown),
            KeyCode::Char('k') if app.ui.selected_index == 2 => Some(Action::MovePackageSelectionUp),
            KeyCode::Char('j') if app.ui.selected_index == 3 => Some(Action::MoveInputSelectionDown),
            KeyCode::Char('k') if app.ui.selected_index == 3 => Some(Action::MoveInputSelectionUp),
            KeyCode::Char('J') => Some(Action::MoveDown),
            KeyCode::Char('K') => Some(Action::MoveUp),
            KeyCode::Down => Some(Action::MoveDown),
            KeyCode::Up => Some(Action::MoveUp),
            KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Char('k') => Some(Action::MoveUp),
            _ => None,
        };
    }
    None
}
