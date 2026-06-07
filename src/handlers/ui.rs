use crate::action::Action;
use crate::context::Context;
use crate::state::AppState;

pub fn handle_ui_action(state: &mut AppState, _context: &Context, action: Action) {
    match action {
        Action::Tick => {
            state.ui.throbber_state.calc_next();
            state.process_background_results();

            if state.ui.is_adding_package
                && state.ui.last_search_time.elapsed() > std::time::Duration::from_millis(300)
                && state.ui.package_search_query != state.ui.last_search_query
            {
                state.ui.last_search_query = state.ui.package_search_query.clone();
                if state.ui.package_search_query.is_empty() {
                    state.domain.package_search_results.clear();
                    state.ui.package_search_state.select(None);
                } else if !state.ui.is_searching_packages {
                    state.perform_package_search();
                }
            }
        }
        Action::NextTab => {
            state.ui.selected_index = if state.mode == crate::state::Mode::Shell {
                1
            } else {
                match state.ui.selected_index {
                    1 => 2,
                    2 => 3,
                    3 => 4,
                    4 => 1,
                    _ => 1,
                }
            };
        }
        Action::PreviousTab => {
            state.ui.selected_index = if state.mode == crate::state::Mode::Shell {
                1
            } else {
                match state.ui.selected_index {
                    1 => 4,
                    2 => 1,
                    3 => 2,
                    4 => 3,
                    _ => 1,
                }
            };
        }
        Action::SelectTab(index) => {
            state.ui.selected_index = index;
        }
        Action::MoveDown => match state.ui.selected_index {
            1 => {
                if state.mode == crate::state::Mode::Shell {
                    let count = state.shell_packages.len();
                    crate::ui::list::ListWrapper::new_table(
                        &mut state.ui.shell_package_list_state,
                        count,
                    )
                    .next();
                }
            }
            2 => {
                if !state.domain.nix_files.is_empty() {
                    state.ui.selected_nix_file_index =
                        (state.ui.selected_nix_file_index + 1) % state.domain.nix_files.len();
                    state.update(Action::RefreshContext);
                }
            }
            3 => {
                state.update(Action::MoveInputSelectionDown);
            }
            4 => {
                if !state.domain.outputs.is_empty() {
                    state.ui.selected_output_index =
                        (state.ui.selected_output_index + 1) % state.domain.outputs.len();
                    state.fetch_package_details();
                }
            }
            5 => {
                if !state.domain.logs.is_empty() {
                    let total_lines =
                        crate::components::command_log::count_lines(&state.domain.logs);
                    crate::ui::list::ListWrapper::new(&mut state.ui.command_log_state, total_lines)
                        .next();
                }
            }
            _ => {}
        },
        Action::MoveUp => match state.ui.selected_index {
            1 => {
                if state.mode == crate::state::Mode::Shell {
                    let count = state.shell_packages.len();
                    crate::ui::list::ListWrapper::new_table(
                        &mut state.ui.shell_package_list_state,
                        count,
                    )
                    .previous();
                }
            }
            2 => {
                if !state.domain.nix_files.is_empty() {
                    state.ui.selected_nix_file_index = if state.ui.selected_nix_file_index == 0 {
                        state.domain.nix_files.len() - 1
                    } else {
                        state.ui.selected_nix_file_index - 1
                    };
                    state.update(Action::RefreshContext);
                }
            }
            3 => {
                state.update(Action::MoveInputSelectionUp);
            }
            4 => {
                if !state.domain.outputs.is_empty() {
                    state.ui.selected_output_index = if state.ui.selected_output_index == 0 {
                        state.domain.outputs.len() - 1
                    } else {
                        state.ui.selected_output_index - 1
                    };
                    state.fetch_package_details();
                }
            }
            5 => {
                if !state.domain.logs.is_empty() {
                    let total_lines =
                        crate::components::command_log::count_lines(&state.domain.logs);
                    crate::ui::list::ListWrapper::new(&mut state.ui.command_log_state, total_lines)
                        .previous();
                }
            }
            _ => {}
        },
        Action::MovePackageSelectionDown => {
            let count = state.domain.package_info.len();
            crate::ui::list::ListWrapper::new_table(&mut state.ui.package_table_state, count)
                .next();
        }
        Action::MovePackageSelectionUp => {
            let count = state.domain.package_info.len();
            crate::ui::list::ListWrapper::new_table(&mut state.ui.package_table_state, count)
                .previous();
        }
        Action::MoveInputSelectionDown => {
            let count = state.domain.inputs.len();
            crate::ui::list::ListWrapper::new_table(&mut state.ui.input_table_state, count).next();
        }
        Action::MoveInputSelectionUp => {
            let count = state.domain.inputs.len();
            crate::ui::list::ListWrapper::new_table(&mut state.ui.input_table_state, count)
                .previous();
        }
        Action::MoveTemplateSelectionDown => {
            let count = state.ui.templates.len();
            if count > 0 {
                let i = match state.ui.template_list_state.selected() {
                    Some(i) => {
                        if i >= count + 1 {
                            2
                        } else {
                            i + 1
                        }
                    }
                    None => 2,
                };
                state.ui.template_list_state.select(Some(i));
            }
        }
        Action::MoveTemplateSelectionUp => {
            let count = state.ui.templates.len();
            if count > 0 {
                let i = match state.ui.template_list_state.selected() {
                    Some(i) => {
                        if i <= 2 {
                            count + 1
                        } else {
                            i - 1
                        }
                    }
                    None => 2,
                };
                state.ui.template_list_state.select(Some(i));
            }
        }
        Action::OpenAddPackage => {
            state.ui.is_adding_package = true;
            state.ui.is_selecting_version = false;
            state.ui.package_search_query.clear();
            state.domain.package_search_results.clear();
            state.domain.package_versions.clear();
        }
        Action::OpenAddInput => {
            state.ui.is_adding_input = true;
            state.ui.new_input_name.clear();
            state.ui.new_input_url.clear();
            state.ui.input_cursor = 1;
            state.start_fetching_suggestions();
        }
        Action::ClosePopup => {
            state.ui.is_adding_package = false;
            state.ui.is_adding_input = false;
            state.ui.is_selecting_version = false;
            state.ui.editing_shell_package_index = None;
        }
        Action::PackageSearchChar(c) => {
            state.ui.package_search_query.push(c);
            state.ui.last_search_time = std::time::Instant::now();
        }
        Action::PackageSearchBackspace => {
            state.ui.package_search_query.pop();
            state.ui.last_search_time = std::time::Instant::now();
        }
        Action::TogglePackageDetails => {
            state.ui.is_showing_package_details = !state.ui.is_showing_package_details;
        }
        Action::MoveSearchSelectionDown => {
            let count = state.domain.package_search_results.len();
            crate::ui::list::ListWrapper::new(&mut state.ui.package_search_state, count).next();
        }
        Action::MoveSearchSelectionUp => {
            let count = state.domain.package_search_results.len();
            crate::ui::list::ListWrapper::new(&mut state.ui.package_search_state, count).previous();
        }
        Action::BackToPackageSearch => {
            if state.ui.editing_shell_package_index.is_some() {
                state.ui.is_adding_package = false;
                state.ui.is_selecting_version = false;
                state.ui.editing_shell_package_index = None;
            } else {
                state.ui.is_selecting_version = false;
            }
        }
        Action::MoveVersionSelectionDown => {
            let inputs_len = if state.mode == crate::state::Mode::Flake {
                if let Some(pkg_name) = &state.ui.selected_package_name {
                    crate::state::domain::get_available_inputs(
                        &state.domain.inputs,
                        pkg_name,
                        &state.domain.package_search_results,
                    )
                    .len()
                } else {
                    0
                }
            } else {
                0
            };
            let total = inputs_len + state.domain.package_versions.len();
            crate::ui::list::ListWrapper::new(&mut state.ui.version_list_state, total).next();
        }
        Action::MoveVersionSelectionUp => {
            let inputs_len = if state.mode == crate::state::Mode::Flake {
                if let Some(pkg_name) = &state.ui.selected_package_name {
                    crate::state::domain::get_available_inputs(
                        &state.domain.inputs,
                        pkg_name,
                        &state.domain.package_search_results,
                    )
                    .len()
                } else {
                    0
                }
            } else {
                0
            };
            let total = inputs_len + state.domain.package_versions.len();
            crate::ui::list::ListWrapper::new(&mut state.ui.version_list_state, total).previous();
        }
        Action::InputPopupChar(c) => {
            match state.ui.input_cursor {
                0 | 1 => state.ui.new_input_name.push(c),
                2 => state.ui.new_input_url.push(c),
                _ => {}
            }
            state.update_suggestions();
        }
        Action::InputPopupBackspace => {
            match state.ui.input_cursor {
                0 | 1 => {
                    state.ui.new_input_name.pop();
                }
                2 => {
                    state.ui.new_input_url.pop();
                }
                _ => {}
            }
            state.update_suggestions();
        }
        Action::NextInputField => {
            state.ui.input_cursor = (state.ui.input_cursor + 1) % 3;
        }
        Action::PreviousInputField => {
            state.ui.input_cursor = if state.ui.input_cursor == 0 {
                2
            } else {
                state.ui.input_cursor - 1
            };
        }
        Action::MoveSuggestionDown => {
            if !state.domain.suggestions.filtered.is_empty() {
                state.domain.suggestions.selected_index = (state.domain.suggestions.selected_index
                    + 1)
                    % state.domain.suggestions.filtered.len();
                state
                    .domain
                    .suggestions
                    .list_state
                    .select(Some(state.domain.suggestions.selected_index));
            }
        }
        Action::MoveSuggestionUp => {
            if !state.domain.suggestions.filtered.is_empty() {
                state.domain.suggestions.selected_index =
                    if state.domain.suggestions.selected_index == 0 {
                        state.domain.suggestions.filtered.len() - 1
                    } else {
                        state.domain.suggestions.selected_index - 1
                    };
                state
                    .domain
                    .suggestions
                    .list_state
                    .select(Some(state.domain.suggestions.selected_index));
            }
        }
        Action::ToggleHelp => {
            state.ui.show_help = !state.ui.show_help;
        }
        Action::ToggleTemplates => {
            state.ui.show_templates = !state.ui.show_templates;
        }
        Action::ToggleSaveShellTemplate => {
            state.ui.is_saving_shell_template = !state.ui.is_saving_shell_template;
            if state.ui.is_saving_shell_template {
                state.ui.new_template_filename = String::new();
                state.ui.new_template_description = String::new();
                state.ui.template_cursor = 0;
            }
        }
        Action::ApplyShellTemplate(pkgs) => {
            for pkg in pkgs {
                let attribute = pkg.clone();
                let pkg_to_add = if let Some(hash) = state.domain.system_nixpkgs_hash.as_ref() {
                    format!("system/{}#{}", hash, attribute)
                } else {
                    attribute.clone()
                };

                if !state.shell_packages.contains(&pkg_to_add) {
                    crate::log_output("Debug", format!("Adding to shell: {}", pkg_to_add));
                    state.shell_packages.push(pkg_to_add.clone());
                    state.fetch_shell_package_metadata(pkg_to_add);
                }
            }
            state.ui.show_templates = false;
        }
        Action::NewTemplateChar(c) => {
            if state.ui.template_cursor == 0 {
                state.ui.new_template_filename.push(c);
            } else {
                state.ui.new_template_description.push(c);
            }
        }
        Action::NewTemplateBackspace => {
            if state.ui.template_cursor == 0 {
                state.ui.new_template_filename.pop();
            } else {
                state.ui.new_template_description.pop();
            }
        }
        _ => {}
    }
}
