pub mod list;

use crate::app::App;
use crate::components::*;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(app: &mut App, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    if app.mode == crate::state::Mode::Shell {
        content::render(app, frame, chunks[0], 1);
    } else {
        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(33), Constraint::Min(0)])
            .split(chunks[0]);

        let col1_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(body_chunks[0]);

        let col2_chunks = if app.ui.selected_index == 5 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0)])
                .split(body_chunks[1])
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
                .split(body_chunks[1])
        };

        title::render(frame, col1_chunks[0], app.ui.selected_index == 1);
        context::render(
            &context::ContextProps {
                nix_files: &app.domain.nix_files,
                selected_nix_file_index: app.ui.selected_nix_file_index,
                is_selected: app.ui.selected_index == 2,
            },
            frame,
            col1_chunks[1],
        );
        inputs::render(
            &inputs::InputsProps {
                inputs: &app.domain.inputs,
                is_selected: app.ui.selected_index == 3,
            },
            frame,
            col1_chunks[2],
        );
        outputs::render(
            &outputs::OutputsProps {
                outputs: &app.domain.outputs,
                is_selected: app.ui.selected_index == 4,
                selected_index: app.ui.selected_output_index,
            },
            frame,
            col1_chunks[3],
        );

        if app.ui.selected_index != 5 {
            content::render(app, frame, col2_chunks[0], app.ui.selected_index);
            command_log::render(
                frame,
                col2_chunks[1],
                &mut command_log::CommandLogProps {
                    is_selected: app.ui.selected_index == 5,
                    logs: &app.domain.logs,
                    state: &mut app.ui.command_log_state,
                    progress: app.domain.nxv_update_progress.as_deref(),
                },
            );
        } else {
            command_log::render(
                frame,
                col2_chunks[0],
                &mut command_log::CommandLogProps {
                    is_selected: app.ui.selected_index == 5,
                    logs: &app.domain.logs,
                    state: &mut app.ui.command_log_state,
                    progress: app.domain.nxv_update_progress.as_deref(),
                },
            );
        }
    }

    let footer_text = if app.mode == crate::state::Mode::Shell {
        "a: Add | d: Remove | i: Info | p: Pin | s: Start Shell | m: Mode | t: Templates | j/k: Select | Space: Multi-select | ?: Help | q: Quit"
    } else {
        match app.ui.selected_index {
            1 => "Tab: Switch focus | m: Switch Mode | t: Templates | 1-5: Select tab | ?: Help | q: Quit",
            2 => "a: Add | d: Remove | i: Info | p: Pin | m: Mode | t: Templates | Shift-j/k: Select | j/k: Navigate pkgs | Space: Multi-select | ?: Help | q: Quit",
            3 => "a: Add Input | m: Switch Mode | t: Templates | Tab: Switch focus | ?: Help | q: Quit",
            4 => "j/k: Select Output | m: Switch Mode | t: Templates | Tab: Switch focus | ?: Help | q: Quit",
            5 => "j/k: Scroll Logs | m: Switch Mode | t: Templates | Tab: Switch focus | ?: Help | q: Quit",
            _ => "Press 'm' to switch mode, 't' for templates, 'Tab' to switch focus, '?' for help, 'q' to quit",
        }
    };

    let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::NONE));
    frame.render_widget(footer, chunks[1]);

    if app.ui.show_help {
        popups::help::render(frame);
    }

    if app.ui.show_templates {
        popups::templates::render(
            frame,
            &mut popups::templates::TemplatesProps {
                table_state: &mut app.ui.template_list_state,
                templates: &app.ui.templates,
            },
        );
    }

    if app.ui.is_saving_shell_template {
        popups::save_shell_template::render(
            frame,
            &app.ui.new_template_filename,
            &app.ui.new_template_description,
            app.ui.template_cursor,
        );
    }

    if app.ui.is_confirming_template_overwrite {
        let name = app.ui.pending_template_name.as_deref().unwrap_or("Template");
        popups::confirm::render(
            frame,
            "Overwrite flake.nix?",
            &format!("Applying '{}' will OVERWRITE your existing flake.nix. Continue?", name),
        );
    }

    if app.ui.is_adding_input {
        popups::add_input::render(
            frame,
            &mut popups::add_input::AddInputProps {
                name: &app.ui.new_input_name,
                url: &app.ui.new_input_url,
                cursor: app.ui.input_cursor,
                suggestions: &mut app.domain.suggestions,
            },
        );
    }

    if app.ui.is_adding_package {
        popups::add_package::render(
            frame,
            &mut popups::add_package::AddPackageProps {
                query: &app.ui.package_search_query,
                results: &app.domain.package_search_results,
                channels: &app.domain.searched_channels,
                is_searching: app.ui.is_searching_packages,
                error: app.ui.package_search_error.as_deref(),
                list_state: &mut app.ui.package_search_state,
                is_selecting_version: app.ui.is_selecting_version,
                is_fetching_versions: app.ui.is_fetching_versions,
                versions: &app.domain.package_versions,
                version_fetch_error: app.ui.version_fetch_error.as_deref(),
                version_list_state: &mut app.ui.version_list_state,
                installed_packages: &app.domain.package_info,
                is_shell_mode: app.mode == crate::state::Mode::Shell,
                inputs: &app.domain.inputs,
                selected_package_name: app.ui.selected_package_name.as_ref(),
                mode: app.mode.clone(),
                system_nixpkgs_version: app.domain.system_nixpkgs_version.as_ref(),
                system_nixpkgs_hash: app.domain.system_nixpkgs_hash.as_ref(),
            },
        );
    }

    if app.ui.is_showing_package_details {
        if app.ui.is_adding_package {
            if let Some(result) = app
                .domain
                .package_search_results
                .get(app.ui.package_search_state.selected().unwrap_or(0))
            {
                popups::add_package::render_details(frame, result);
            }
        } else if app.ui.selected_index == 2 || (app.mode == crate::state::Mode::Shell && app.ui.selected_index == 1) {
            let mut all_packages = Vec::new();
            if app.mode == crate::state::Mode::Shell {
                for p in &app.shell_packages {
                    let mut name = p.clone();
                    let mut version = "Unknown".to_string();
                    if p.contains('@') {
                        if let Some((rest, v)) = p.rsplit_once('@') {
                            version = v.to_string();
                            if (rest.starts_with("nixpkgs/") || rest.starts_with("system/")) && rest.contains('#') {
                                if let Some((_, suffix)) = rest.split_once('#') {
                                    name = suffix.to_string();
                                }
                            } else {
                                name = rest.to_string();
                            }
                        }
                    } else if (p.starts_with("nixpkgs/") || p.starts_with("system/")) && p.contains('#') {
                        if let Some((_, suffix)) = p.split_once('#') {
                            name = suffix.to_string();
                        }
                    }

                    all_packages.push(crate::state::domain::SearchResult {
                        name: name.clone(),
                        description: "".to_string(),
                        versions: vec![crate::state::domain::ChannelVersion {
                            version: version.clone(),
                            channel: "shell".to_string(),
                            locked_version: None,
                        }],
                        platforms: Vec::new(),
                        is_unfree: false,
                        source_input: None,
                        hash: None,
                    });
                }
            } else {
                for (name, (description, version, is_unfree, source_input)) in &app.domain.package_info {
                    all_packages.push(crate::state::domain::SearchResult {
                        name: name.clone(),
                        description: description.clone(),
                        versions: vec![crate::state::domain::ChannelVersion {
                            version: version.clone(),
                            channel: "current".to_string(),
                            locked_version: None,
                        }],
                        platforms: Vec::new(),
                        is_unfree: *is_unfree,
                        source_input: if source_input.is_empty() {
                            None
                        } else {
                            Some(source_input.clone())
                        },
                        hash: None,
                    });
                }
                all_packages.sort_by(|a, b| a.name.cmp(&b.name));
            }

            let selected_idx = if app.mode == crate::state::Mode::Shell {
                app.ui.shell_package_list_state.selected()
            } else {
                app.ui.package_table_state.selected()
            };

            if let Some(i) = selected_idx {
                if i > 0 {
                    if let Some(result) = all_packages.get(i - 1) {
                        popups::add_package::render_details(frame, result);
                    }
                }
            }
        }
    }
}
