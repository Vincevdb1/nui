use crate::app::{App, Effect, Mode, Popup};
use crate::backend::templates::Template;
use crate::pkgref::Source;
use crate::ui::widgets::{Selectable, footer, popup_frame, ruled_header, text_field};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};

pub struct TemplatePicker {
    pub table: TableState,
}

impl TemplatePicker {
    pub fn new(templates: &[Template]) -> Self {
        let mut table = TableState::default();
        table.select((!templates.is_empty()).then_some(0));
        Self { table }
    }
}

pub fn open(app: &mut App) {
    app.popup = Popup::Templates(TemplatePicker::new(&app.templates));
}

pub fn picker_key(app: &mut App, mut picker: TemplatePicker, key: KeyEvent) -> Popup {
    let len = app.templates.len();
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('t') => return Popup::None,
        KeyCode::Down | KeyCode::Char('j') => picker.table.step(len, true),
        KeyCode::Up | KeyCode::Char('k') => picker.table.step(len, false),
        KeyCode::Enter => {
            let Some(template) = picker.table.selected().and_then(|i| app.templates.get(i)) else {
                return Popup::None;
            };
            let name = template.name.clone();
            if app.mode == Mode::Flake && std::path::Path::new("flake.nix").exists() {
                return Popup::ConfirmTemplate(name);
            }
            apply(app, name);
            return Popup::None;
        }
        _ => {}
    }
    Popup::Templates(picker)
}

fn apply(app: &mut App, name: String) {
    app.effects.push(Effect::ApplyTemplate {
        name,
        shell: app.mode == Mode::Shell,
    });
}

pub fn confirm_key(app: &mut App, name: String, key: KeyEvent) -> Popup {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => {
            apply(app, name);
            Popup::None
        }
        KeyCode::Char('n') | KeyCode::Esc => Popup::None,
        _ => Popup::ConfirmTemplate(name),
    }
}

#[derive(Default)]
pub struct SaveTemplate {
    pub filename: String,
    pub description: String,
    pub on_description: bool,
}

pub fn open_save(app: &mut App) {
    app.popup = Popup::SaveTemplate(SaveTemplate::default());
}

pub fn save_key(app: &mut App, mut form: SaveTemplate, key: KeyEvent) -> Popup {
    let field = if form.on_description {
        &mut form.description
    } else {
        &mut form.filename
    };
    match key.code {
        KeyCode::Esc => return Popup::None,
        KeyCode::Tab | KeyCode::BackTab => form.on_description = !form.on_description,
        KeyCode::Char(c) => field.push(c),
        KeyCode::Backspace => {
            field.pop();
        }
        KeyCode::Enter if !form.filename.trim().is_empty() => {
            app.effects.push(Effect::SaveTemplate {
                name: form.filename,
                description: form.description,
                packages: app
                    .shell
                    .packages
                    .iter()
                    // Templates only know nixpkgs attributes.
                    .filter(|p| !matches!(p.reference.source, Source::Flake { .. }))
                    .map(|p| p.reference.attr.clone())
                    .collect(),
            });
            return Popup::None;
        }
        _ => {}
    }
    Popup::SaveTemplate(form)
}

pub fn render_picker(frame: &mut Frame, picker: &mut TemplatePicker, templates: &[Template]) {
    let area = popup_frame(frame, 80, 60, "Select Template", Color::Magenta);
    let [body, hint] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
        .margin(2)
        .areas(area);

    if templates.is_empty() {
        frame.render_widget(
            Paragraph::new("No templates found in ~/.config/nui/templates/")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray)),
            body,
        );
    } else {
        let rows = templates.iter().map(|t| {
            Row::new(vec![
                Cell::from(format!("  {}", t.name)),
                Cell::from("│"),
                Cell::from(format!(" {}", t.description)),
            ])
        });
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(30),
                Constraint::Length(1),
                Constraint::Min(20),
            ],
        )
        .header(
            ruled_header(&[" Template Name", "│", " Description"])
                .style(Style::default().fg(Color::Cyan)),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::Rgb(50, 50, 50))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
        frame.render_stateful_widget(table, body, &mut picker.table);
    }

    frame.render_widget(
        footer("Enter: Use Template | Esc: Skip / Close").alignment(Alignment::Center),
        hint,
    );
}

pub fn render_confirm(frame: &mut Frame, name: &str) {
    let area = popup_frame(frame, 50, 25, "Overwrite flake.nix?", Color::Red);
    let [body, hint] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
        .margin(2)
        .areas(area);
    frame.render_widget(
        Paragraph::new(format!(
            "Applying '{}' will OVERWRITE your existing flake.nix. Continue?",
            name
        ))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        body,
    );
    frame.render_widget(
        footer("y: Yes / Confirm | n / Esc: No / Cancel").alignment(Alignment::Center),
        hint,
    );
}

pub fn render_save(frame: &mut Frame, form: &SaveTemplate) {
    let area = popup_frame(frame, 60, 40, "Save Shell as Template", Color::Yellow);
    let [filename, description, info, hint] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .margin(2)
    .areas(area);

    frame.render_widget(
        text_field(&form.filename, " Filename ", !form.on_description),
        filename,
    );
    frame.render_widget(
        text_field(&form.description, " Description ", form.on_description),
        description,
    );
    frame.render_widget(
        Paragraph::new("Packages to include: default devShell with current shell packages.")
            .style(Style::default().fg(Color::Gray))
            .wrap(Wrap { trim: true }),
        info,
    );
    frame.render_widget(
        footer(if form.on_description {
            "Type Description... | Enter: Save Template | Tab: Previous Field | Esc: Close"
        } else {
            "Type Filename... | Tab: Next Field | Esc: Close"
        }),
        hint,
    );
}
