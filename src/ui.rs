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
        // Full screen (minus footer) for shell packages
        content::render(app, frame, chunks[0], 1);
    } else {
        // Create the 2nd row layout (2 columns)
        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(33), Constraint::Min(0)])
            .split(chunks[0]);

        // Column 1: 4 rows (Title + 3 even rows)
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
        context::render(app, frame, col1_chunks[1], app.ui.selected_index == 2);
        inputs::render(
            &app.domain.inputs,
            frame,
            col1_chunks[2],
            app.ui.selected_index == 3,
        );
        configurations::render(
            &app.domain.configurations,
            frame,
            col1_chunks[3],
            app.ui.selected_index == 4,
            app.ui.selected_configuration_index,
        );

        if app.ui.selected_index != 5 {
            content::render(app, frame, col2_chunks[0], app.ui.selected_index);
            command_log::render(
                frame,
                col2_chunks[1],
                app.ui.selected_index == 5,
                &app.domain.logs,
                &mut app.ui.command_log_state,
            );
        } else {
            command_log::render(
                frame,
                col2_chunks[0],
                app.ui.selected_index == 5,
                &app.domain.logs,
                &mut app.ui.command_log_state,
            );
        }
    }

    let footer_text = if app.mode == crate::state::Mode::Shell {
        "a: Add | x: Remove | s: Start Shell | m: Switch Mode | j/k: Select | q: Quit"
    } else {
        match app.ui.selected_index {
            1 => "Tab: Switch focus | m: Switch Mode | 1-5: Select tab | q: Quit",
            2 => "a: Add Package | m: Switch Mode | j/k: Select File | Tab: Switch focus | 1-5: Select tab | q: Quit",
            3 => "a: Add Input | m: Switch Mode | Tab: Switch focus | 1-5: Select tab | q: Quit",
            4 => "j/k: Select Config | m: Switch Mode | Tab: Switch focus | 1-5: Select tab | q: Quit",
            5 => "j/k: Scroll Logs | m: Switch Mode | Tab: Switch focus | 1-5: Select tab | q: Quit",
            _ => "Press 'm' to switch mode, 'Tab' to switch focus, 'q' to quit",
        }
    };

    let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::NONE));
    frame.render_widget(footer, chunks[1]);

    if app.ui.is_adding_input {
        popups::add_input::render(
            frame,
            &app.ui.new_input_name,
            &app.ui.new_input_url,
            app.ui.input_cursor,
            &mut app.domain.suggestions,
        );
    }

    if app.ui.is_adding_package {
        popups::add_package::render(
            frame,
            &app.ui.package_search_query,
            &app.domain.package_search_results,
            &app.domain.searched_channels,
            app.ui.is_searching_packages,
            &mut app.ui.package_search_state,
            app.ui.is_selecting_version,
            app.ui.is_fetching_versions,
            &app.domain.package_versions,
            &mut app.ui.version_list_state,
        );
    }

    if app.ui.is_showing_package_details
        && let Some(result) = app
            .domain
            .package_search_results
            .get(app.ui.package_search_state.selected().unwrap_or(0))
    {
        popups::add_package::render_details(frame, result);
    }
}
