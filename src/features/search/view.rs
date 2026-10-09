use super::versions::{VersionItem, VersionPicker};
use super::{Search, Stage};
use crate::backend::search::SearchResult;
use crate::features::details;
use crate::flake::Package;
use crate::ui::widgets::{
    channel_color, footer, highlight, placeholder, popup_frame, scrollbar, truncate, unfree_marker,
};
use ratatui::{prelude::*, widgets::*};

pub struct Context<'a> {
    pub shell_mode: bool,
    pub installed: &'a [Package],
    pub system_version: Option<&'a str>,
    pub system_rev: Option<&'a str>,
}

pub fn render(frame: &mut Frame, s: &mut Search, ctx: &Context) {
    let replacing = matches!(&s.stage, Stage::Versions(p) if p.replacing.is_some());
    let area = popup_frame(
        frame,
        80,
        70,
        if replacing {
            "Change Version"
        } else {
            "Add Package"
        },
        Color::Yellow,
    );
    let [query, body, hint] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .margin(2)
    .areas(area);

    frame.render_widget(
        Paragraph::new(s.query.as_str()).block(
            Block::default()
                .title(" Search Query ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        query,
    );

    let hint_text = match &s.stage {
        Stage::Versions(_) => "Enter: Select Version | Esc: Back to Search",
        _ => "Type: Search | Enter: Add | M-Enter: Versions | Tab: Details | Esc: Close",
    };
    frame.render_widget(footer(hint_text), hint);

    match &mut s.stage {
        Stage::Versions(picker) => render_versions(frame, body, picker),
        Stage::Results | Stage::Details => render_results(frame, body, s, ctx),
    }
    if matches!(s.stage, Stage::Details)
        && let Some(result) = s.selected()
    {
        details::render(frame, result);
    }
}

fn list_with_header(
    frame: &mut Frame,
    area: Rect,
    header: Line,
    items: Vec<ListItem>,
    title: &str,
    state: &mut ListState,
) {
    let [header_area, list_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    frame.render_widget(Paragraph::new(header), header_area);
    let len = items.len();
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .title(title.to_string())
                    .borders(Borders::ALL),
            )
            .highlight_style(highlight())
            .highlight_symbol(">> "),
        list_area,
        state,
    );
    scrollbar(frame, list_area, len, state.selected());
}

fn bold(text: String) -> Span<'static> {
    Span::styled(text, Style::default().add_modifier(Modifier::BOLD))
}

fn description(result: &SearchResult) -> Span<'static> {
    Span::styled(
        truncate(&result.description, 60),
        Style::default().fg(Color::DarkGray),
    )
}

// One leading column covers the border, the ">> " highlight symbol and the unfree marker.
const GUTTER: &str = "      ";

fn render_results(frame: &mut Frame, area: Rect, s: &mut Search, ctx: &Context) {
    let title = if s.searching {
        " Searching... "
    } else {
        " Search Results (Enter to select) "
    };
    if s.results.is_empty() {
        frame.render_widget(Block::default().title(title).borders(Borders::ALL), area);
        placeholder(
            frame,
            area,
            "No results found.",
            s.error.as_deref().map(|e| ("searching packages", e)),
        );
        return;
    }

    let name_width = s
        .results
        .iter()
        .map(|r| r.name.len())
        .max()
        .unwrap_or(0)
        .max(10)
        + 1;
    let (header, items) = if ctx.shell_mode {
        shell_columns(&s.results, name_width, ctx)
    } else {
        channel_columns(&s.results, &s.channels, name_width, ctx.installed)
    };
    list_with_header(frame, area, header, items, title, &mut s.list);
}

fn shell_columns(
    results: &[SearchResult],
    name_width: usize,
    ctx: &Context,
) -> (Line<'static>, Vec<ListItem<'static>>) {
    let version = |r: &SearchResult| {
        let mut v = r.shell_version(ctx.system_version);
        if r.hash.as_deref().unwrap_or("") != ctx.system_rev.unwrap_or("") {
            v.push_str(" 󰚰");
        }
        v
    };
    let version_width = results
        .iter()
        .map(|r| version(r).chars().count())
        .max()
        .unwrap_or(0)
        .max(10);

    let header = Line::from(vec![
        Span::raw(GUTTER),
        bold(format!("{:name_width$}", "Name")),
        Span::raw(" | "),
        Span::styled(
            format!("{:version_width$}", "Version"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        bold("Description".to_string()),
    ]);
    let items = results
        .iter()
        .map(|r| {
            ListItem::new(Line::from(vec![
                unfree_marker(r.is_unfree),
                bold(format!("{:name_width$}", r.name)),
                Span::raw(" | "),
                Span::styled(
                    format!("{:version_width$}", version(r)),
                    Style::default().fg(Color::Green),
                ),
                Span::raw(" | "),
                description(r),
            ]))
        })
        .collect();
    (header, items)
}

fn channel_columns(
    results: &[SearchResult],
    channels: &[String],
    name_width: usize,
    installed: &[Package],
) -> (Line<'static>, Vec<ListItem<'static>>) {
    let widths: Vec<usize> = channels
        .iter()
        .map(|channel| {
            results
                .iter()
                .filter_map(|r| r.versions.iter().find(|v| v.channel == *channel))
                .map(|v| v.version.len() + v.locked_version.as_ref().map_or(0, |l| l.len() + 3) + 4)
                .max()
                .unwrap_or(0)
                .max(channel.len())
                .max(5)
        })
        .collect();

    let mut header = vec![Span::raw(GUTTER), bold(format!("{:name_width$}", "Name"))];
    for (channel, width) in channels.iter().zip(&widths) {
        header.push(Span::raw(" | "));
        header.push(Span::styled(
            format!("{:width$}", channel),
            Style::default()
                .fg(channel_color(channel))
                .add_modifier(Modifier::BOLD),
        ));
    }
    header.push(Span::raw(" | "));
    header.push(bold("Description".to_string()));

    let is_installed = |name: &str| {
        installed
            .iter()
            .any(|p| p.name == name || name.ends_with(&format!(".{}", p.name)))
    };

    let items = results
        .iter()
        .map(|r| {
            let name_style = if is_installed(&r.name) {
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan)
            } else {
                Style::default().add_modifier(Modifier::BOLD)
            };
            let mut spans = vec![
                unfree_marker(r.is_unfree),
                Span::styled(format!("{:name_width$}", r.name), name_style),
            ];
            for (channel, width) in channels.iter().zip(&widths) {
                spans.push(Span::raw(" | "));
                spans.push(match r.versions.iter().find(|v| v.channel == *channel) {
                    Some(v) => {
                        let text = match &v.locked_version {
                            Some(locked) if *locked != v.version => {
                                format!("{} (󰌾 {})", v.version, locked)
                            }
                            _ => v.version.clone(),
                        };
                        Span::styled(
                            format!("{:width$}", text),
                            Style::default().fg(channel_color(channel)),
                        )
                    }
                    None => Span::raw(" ".repeat(*width)),
                });
            }
            spans.push(Span::raw(" | "));
            spans.push(description(r));
            ListItem::new(Line::from(spans))
        })
        .collect();
    (Line::from(header), items)
}

fn render_versions(frame: &mut Frame, area: Rect, picker: &mut VersionPicker) {
    let title = if picker.loading {
        " Searching for versions... (󱑆) "
    } else {
        " Select Input or Version (Enter to add) "
    };
    if picker.items.is_empty() {
        frame.render_widget(Block::default().title(title).borders(Borders::ALL), area);
        let message = if picker.loading {
            "Fetching versions from nxv..."
        } else {
            "No versions found."
        };
        let error = (!picker.loading)
            .then_some(picker.error.as_deref())
            .flatten()
            .map(|e| ("fetching versions", e));
        placeholder(frame, area, message, error);
        return;
    }

    let header = Line::from(vec![
        Span::raw(GUTTER),
        bold(format!("{:30}", "Version (Input)")),
        Span::raw(" | "),
        bold("Hash".to_string()),
    ]);
    let items = picker
        .items
        .iter()
        .map(|item| {
            let (marker, label, style, hash) = match item {
                VersionItem::Input {
                    input,
                    channel,
                    version,
                    rev,
                } => (
                    unfree_marker(false),
                    format!("{} ({})", version, input),
                    Style::default().fg(channel_color(channel)),
                    rev.clone(),
                ),
                VersionItem::Version(v) if v.is_system => (
                    unfree_marker(v.is_unfree),
                    format!("{} [System]", v.version),
                    Style::default().fg(Color::Yellow),
                    v.hash.clone(),
                ),
                VersionItem::Version(v) => (
                    unfree_marker(v.is_unfree),
                    v.version.clone(),
                    Style::default(),
                    v.hash.clone(),
                ),
            };
            ListItem::new(Line::from(vec![
                marker,
                Span::styled(format!("{:30}", label), style.add_modifier(Modifier::BOLD)),
                Span::raw(" | "),
                Span::styled(hash, Style::default().fg(Color::Cyan)),
            ]))
        })
        .collect();
    list_with_header(frame, area, header, items, title, &mut picker.list);
}
