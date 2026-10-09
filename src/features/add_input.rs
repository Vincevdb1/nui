use crate::app::effect::FlakeEdit;
use crate::app::{App, Effect, Popup};
use crate::backend::github::Branch;
use crate::flake::Input;
use crate::ui::widgets::{Selectable, footer, placeholder, popup_frame, text_field};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Suggestions,
    Name,
    Url,
}

impl Field {
    fn cycle(self, forward: bool) -> Self {
        match (self, forward) {
            (Self::Suggestions, true) | (Self::Url, false) => Self::Name,
            (Self::Name, true) | (Self::Suggestions, false) => Self::Url,
            (Self::Url, true) | (Self::Name, false) => Self::Suggestions,
        }
    }
}

pub struct AddInput {
    pub name: String,
    pub url: String,
    pub field: Field,
    branches: Vec<Branch>,
    pub suggestions: Vec<Branch>,
    pub list: ListState,
    pub loading: bool,
    pub error: Option<String>,
    /// A package to add from the input once it is declared.
    package: Option<String>,
}

impl AddInput {
    fn filter(&mut self, inputs: &[Input]) {
        let query = self.name.to_lowercase();
        self.suggestions = self
            .branches
            .iter()
            .filter(|b| b.name.to_lowercase().contains(&query))
            .filter(|b| !inputs.iter().any(|i| i.url == b.url))
            .cloned()
            .collect();
        let selected = self.list.selected().filter(|i| *i < self.suggestions.len());
        self.list.select(if self.suggestions.is_empty() {
            None
        } else {
            Some(selected.unwrap_or(0))
        });
    }

    pub fn branches_loaded(&mut self, result: Result<Vec<Branch>, String>, inputs: &[Input]) {
        self.loading = false;
        match result {
            Ok(branches) => {
                self.branches = branches;
                self.error = None;
                self.filter(inputs);
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn field_mut(&mut self) -> &mut String {
        match self.field {
            Field::Suggestions | Field::Name => &mut self.name,
            Field::Url => &mut self.url,
        }
    }
}

/// `prefill` is a name, url and package to add once the input exists.
pub fn open(app: &mut App, prefill: Option<(String, String, String)>) -> Popup {
    let (name, url, package) = match prefill {
        Some((name, url, package)) => (name, url, Some(package)),
        None => Default::default(),
    };
    app.effects.push(Effect::FetchBranches);
    Popup::AddInput(AddInput {
        name,
        url,
        field: Field::Name,
        branches: Vec::new(),
        suggestions: Vec::new(),
        list: ListState::default(),
        loading: true,
        error: None,
        package,
    })
}

pub fn on_key(app: &mut App, mut form: AddInput, key: KeyEvent) -> Popup {
    let on_suggestions = form.field == Field::Suggestions;
    match key.code {
        KeyCode::Esc => return Popup::None,
        KeyCode::Tab => form.field = form.field.cycle(true),
        KeyCode::BackTab => form.field = form.field.cycle(false),
        KeyCode::Down | KeyCode::Char('j') if on_suggestions => {
            form.list.step(form.suggestions.len(), true)
        }
        KeyCode::Up | KeyCode::Char('k') if on_suggestions => {
            form.list.step(form.suggestions.len(), false)
        }
        KeyCode::Down => form.field = form.field.cycle(true),
        KeyCode::Up => form.field = form.field.cycle(false),
        KeyCode::Char(c) => {
            form.field_mut().push(c);
            form.filter(&app.flake.inputs);
        }
        KeyCode::Backspace => {
            form.field_mut().pop();
            form.filter(&app.flake.inputs);
        }
        KeyCode::Enter if submit(app, &form) => return Popup::None,
        _ => {}
    }
    Popup::AddInput(form)
}

/// Adds the highlighted suggestion, or the typed name and url.
fn submit(app: &mut App, form: &AddInput) -> bool {
    let suggestion = form
        .list
        .selected()
        .and_then(|i| form.suggestions.get(i))
        .filter(|_| form.field == Field::Suggestions);
    let (name, url) = match suggestion {
        Some(branch) => (branch.name.clone(), branch.url.clone()),
        None => (form.name.trim().to_string(), form.url.trim().to_string()),
    };
    if name.is_empty() || url.is_empty() {
        return false;
    }
    let Some(file) = app.flake.selected_file().cloned() else {
        return true;
    };
    let package = match &form.package {
        Some(pkg) => app
            .flake
            .edit_target()
            .map(|(_, target)| (target, pkg.clone())),
        None => None,
    };
    app.effects.push(Effect::Edit {
        flake: file,
        edit: FlakeEdit::AddInput { name, url, package },
    });
    true
}

pub fn render(frame: &mut Frame, form: &mut AddInput) {
    let area = popup_frame(frame, 80, 70, "Add Input", Color::Yellow);
    let [list_area, manual, hint] = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(10),
        Constraint::Length(1),
    ])
    .margin(2)
    .areas(area);

    let on_list = form.field == Field::Suggestions;
    let list_style = if on_list {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let title = if form.loading {
        " Suggestions (Loading...) "
    } else {
        " Suggestions (Enter to add) "
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(list_style);

    if form.loading || form.suggestions.is_empty() {
        frame.render_widget(block, list_area);
        let message = if form.loading {
            "Loading branches from GitHub..."
        } else {
            "No suggestions found."
        };
        let error = (!form.loading)
            .then_some(form.error.as_deref())
            .flatten()
            .map(|e| ("fetching suggestions", e));
        placeholder(frame, list_area, message, error);
    } else {
        let items: Vec<ListItem> = form
            .suggestions
            .iter()
            .map(|b| ListItem::new(format!("  {}", b.name)))
            .collect();
        let highlight = if on_list {
            crate::ui::widgets::highlight()
        } else {
            Style::default().bg(Color::DarkGray)
        };
        frame.render_stateful_widget(
            List::new(items)
                .block(block)
                .highlight_style(highlight)
                .highlight_symbol(">> "),
            list_area,
            &mut form.list,
        );
    }

    let manual_block = Block::default()
        .title(" Manual Input ")
        .borders(Borders::ALL)
        .border_style(if on_list {
            Style::default()
        } else {
            Style::default().fg(Color::Cyan)
        });
    frame.render_widget(manual_block, manual);
    let [name, url] = Layout::vertical([Constraint::Length(3), Constraint::Length(3)])
        .margin(1)
        .areas(manual);
    frame.render_widget(
        text_field(&form.name, " Name ", form.field == Field::Name),
        name,
    );
    frame.render_widget(
        text_field(&form.url, " URL ", form.field == Field::Url),
        url,
    );

    frame.render_widget(
        footer(match form.field {
            Field::Suggestions => {
                "Enter: Select | j/k: Nav Suggestions | Tab: Next Field | Esc: Close"
            }
            Field::Name => "Type Input Name... | Tab: Next Field | Esc: Close",
            Field::Url => "Type Input URL... | Enter: Add | Tab: Next Field | Esc: Close",
        }),
        hint,
    );
}
