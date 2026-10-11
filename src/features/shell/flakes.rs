use super::flake_name;
use crate::app::{App, Popup};
use crate::flake::Input;
use crate::ui::widgets::{Selectable, footer, highlight, placeholder, popup_frame, text_field};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};

/// The flakes searched alongside nixpkgs, with an optional flake reference being typed.
#[derive(Default)]
pub struct FlakeList {
    pub list: ListState,
    pub input: Option<String>,
    pub error: Option<String>,
}

pub fn open(app: &mut App) {
    let mut list = ListState::default();
    list.select((!app.shell.flakes.is_empty()).then_some(0));
    app.popup = Popup::ShellFlakes(FlakeList {
        list,
        ..Default::default()
    });
}

pub fn on_key(app: &mut App, mut form: FlakeList, key: KeyEvent) -> Popup {
    let flakes = &mut app.shell.flakes;
    match (&mut form.input, key.code) {
        (Some(_), KeyCode::Esc) => form.input = None,
        (Some(url), KeyCode::Char(c)) => url.push(c),
        (Some(url), KeyCode::Backspace) => {
            url.pop();
        }
        (Some(url), KeyCode::Enter) => {
            let url = url.trim();
            if flakes.iter().any(|f| f.url == url) {
                form.error = Some(format!("{} is already in the list", url));
                form.input = None;
            } else if !url.is_empty() {
                flakes.push(Input {
                    name: unique_name(flakes, url),
                    url: url.to_string(),
                    ..Default::default()
                });
                form.list.select(Some(flakes.len() - 1));
                form.error = None;
                form.input = None;
            }
        }
        (Some(_), _) => {}
        (None, KeyCode::Esc | KeyCode::Char('q' | 'f')) => return Popup::None,
        (None, KeyCode::Char('a')) => form.input = Some(String::new()),
        (None, KeyCode::Char('d')) => {
            if let Some(i) = form.list.selected().filter(|i| *i < flakes.len()) {
                flakes.remove(i);
                form.list.select(match flakes.len() {
                    0 => None,
                    len => Some(i.min(len - 1)),
                });
            }
        }
        (None, KeyCode::Char('j') | KeyCode::Down) => form.list.step(flakes.len(), true),
        (None, KeyCode::Char('k') | KeyCode::Up) => form.list.step(flakes.len(), false),
        (None, _) => {}
    }
    Popup::ShellFlakes(form)
}

/// `flake_name(url)`, numbered when the name is taken or means something to the search.
fn unique_name(flakes: &[Input], url: &str) -> String {
    let base = flake_name(url);
    let taken =
        |name: &str| matches!(name, "nxv" | "system") || flakes.iter().any(|f| f.name == name);
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{}-{}", base, n))
        .find(|name| !taken(name))
        .unwrap_or(base)
}

pub fn render(frame: &mut Frame, form: &mut FlakeList, flakes: &[Input]) {
    let area = popup_frame(frame, 70, 50, "Search Flakes", Color::Yellow);
    let [list_area, input_area, hint] = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .margin(2)
    .areas(area);

    let block = Block::default()
        .title(" Flakes searched with nixpkgs ")
        .borders(Borders::ALL);
    if flakes.is_empty() {
        frame.render_widget(block, list_area);
        placeholder(frame, list_area, "No flakes added yet.", None);
    } else {
        let name_width = flakes.iter().map(|f| f.name.len()).max().unwrap_or(0);
        let items: Vec<ListItem> = flakes
            .iter()
            .map(|f| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:name_width$}", f.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" | "),
                    Span::styled(f.url.clone(), Style::default().fg(Color::Cyan)),
                ]))
            })
            .collect();
        frame.render_stateful_widget(
            List::new(items)
                .block(block)
                .highlight_style(highlight())
                .highlight_symbol(">> "),
            list_area,
            &mut form.list,
        );
    }

    match (&form.input, &form.error) {
        (Some(url), _) => frame.render_widget(text_field(url, " Flake URL ", true), input_area),
        (None, Some(error)) => frame.render_widget(
            Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red)),
            input_area,
        ),
        (None, None) => {}
    }

    frame.render_widget(
        footer(if form.input.is_some() {
            "Type Flake URL (e.g. github:owner/repo)... | Enter: Add | Esc: Back"
        } else {
            "a: Add Flake | d: Remove Flake | j/k: Select | Esc: Close"
        }),
        hint,
    );
}
