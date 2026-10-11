pub mod keys;
pub mod versions;
pub mod view;

pub use keys::on_key;

use crate::app::{App, Effect, Generation, Mode, Popup};
use crate::backend::nxv::VersionInfo;
use crate::backend::search::{self, ChannelVersion, SearchResult};
use crate::flake::Input;
use ratatui::widgets::ListState;
use std::time::{Duration, Instant};
use versions::VersionPicker;

const DEBOUNCE: Duration = Duration::from_millis(300);

pub enum Stage {
    Results,
    Details,
    Versions(VersionPicker),
}

pub struct Search {
    pub query: String,
    searched: String,
    edited: Instant,
    pub results: Vec<SearchResult>,
    pub channels: Vec<String>,
    pub list: ListState,
    pub searching: bool,
    pub error: Option<String>,
    pub stage: Stage,
}

impl Search {
    fn new(stage: Stage) -> Self {
        Self {
            query: String::new(),
            searched: String::new(),
            edited: Instant::now(),
            results: Vec::new(),
            channels: Vec::new(),
            list: ListState::default(),
            searching: false,
            error: None,
            stage,
        }
    }

    pub fn edit(&mut self, f: impl FnOnce(&mut String)) {
        f(&mut self.query);
        self.edited = Instant::now();
    }

    pub fn selected(&self) -> Option<&SearchResult> {
        self.results.get(self.list.selected()?)
    }

    fn channel_versions(&self, package: &str) -> &[ChannelVersion] {
        self.results
            .iter()
            .find(|r| r.name == package)
            .map(|r| r.versions.as_slice())
            .unwrap_or(&[])
    }
}

/// Supersedes any running search so its late results are ignored.
fn cancel(app: &mut App) {
    let generation = app.search_generation.bump();
    app.effects.push(Effect::CancelSearch(generation));
}

pub fn open(app: &mut App) {
    cancel(app);
    app.popup = Popup::Search(Search::new(Stage::Results));
}

/// Opens the version picker to swap the version of a shell package.
pub fn open_versions(app: &mut App, index: usize) {
    let Some(pkg) = app.shell.packages.get(index) else {
        return;
    };
    let attr = pkg.reference.attr.clone();
    let picker = VersionPicker::new(
        attr.clone(),
        pkg.description.clone(),
        pkg.is_unfree,
        Some(index),
    );
    cancel(app);
    app.popup = Popup::Search(Search::new(Stage::Versions(picker)));
    app.effects.push(Effect::FetchVersions(attr));
}

pub fn close(app: &mut App) -> Popup {
    cancel(app);
    Popup::None
}

pub fn tick(app: &mut App) {
    let Popup::Search(s) = &mut app.popup else {
        return;
    };
    if s.edited.elapsed() <= DEBOUNCE || s.query == s.searched {
        return;
    }
    if s.query.is_empty() {
        s.searched.clear();
        s.results.clear();
        s.list.select(None);
        return;
    }
    let shell_mode = app.mode == Mode::Shell;
    // Shell searches target the system's channel, so wait until it is known.
    if shell_mode && app.system.is_none() {
        return;
    }

    s.searched = s.query.clone();
    s.searching = true;
    s.error = None;
    let system_version = app
        .system
        .as_ref()
        .and_then(|i| i.nixpkgs_version.as_deref());
    let inputs = if shell_mode {
        &app.shell.flakes
    } else {
        &app.flake.inputs
    };
    let request = search::Request::new(s.query.clone(), shell_mode, inputs, system_version);
    s.channels = request.channels();
    app.effects.push(Effect::Search {
        generation: app.search_generation.bump(),
        request,
    });
}

pub fn finished(app: &mut App, generation: Generation, result: Result<Vec<SearchResult>, String>) {
    let Popup::Search(s) = &mut app.popup else {
        return;
    };
    if generation != app.search_generation {
        return;
    }
    s.searching = false;
    match result {
        Ok(results) => {
            s.error = None;
            s.results = results;
            s.list
                .select(if s.results.is_empty() { None } else { Some(0) });
            if app.mode == Mode::Flake && !s.results.is_empty() {
                app.effects.push(Effect::LockedVersions {
                    generation,
                    results: s.results.iter().take(10).cloned().collect(),
                    inputs: app.flake.inputs.clone(),
                });
            }
        }
        Err(e) => {
            crate::log::output("Nix Error", format!("Error searching packages: {}", e));
            s.error = Some(e);
        }
    }
}

fn flake_inputs(app_mode: Mode, inputs: &[Input]) -> &[Input] {
    if app_mode == Mode::Flake { inputs } else { &[] }
}

pub fn locked_version(
    app: &mut App,
    generation: Generation,
    attr: &str,
    channel: &str,
    version: String,
) {
    let Popup::Search(s) = &mut app.popup else {
        return;
    };
    if generation != app.search_generation {
        return;
    }
    for cv in s
        .results
        .iter_mut()
        .filter(|r| r.name == attr)
        .flat_map(|r| r.versions.iter_mut())
        .filter(|cv| cv.channel == channel)
    {
        cv.locked_version = Some(version.clone());
    }
    if let Stage::Versions(picker) = &mut s.stage
        && picker.package == attr
    {
        let versions = s
            .results
            .iter()
            .find(|r| r.name == attr)
            .map(|r| r.versions.as_slice())
            .unwrap_or(&[]);
        picker.rebuild(flake_inputs(app.mode, &app.flake.inputs), versions);
    }
}

/// Newer versions found for installed packages also refresh matching search columns.
pub fn latest_version(app: &mut App, attr: &str, version: &str) {
    let Popup::Search(s) = &mut app.popup else {
        return;
    };
    for cv in s
        .results
        .iter_mut()
        .filter(|r| r.name == attr)
        .flat_map(|r| r.versions.iter_mut())
        .filter(|cv| {
            cv.channel.contains("nixpkgs") || cv.channel.contains("nixos") || cv.channel.len() == 40
        })
    {
        cv.version = version.to_string();
    }
}

pub fn versions_loaded(app: &mut App, attr: &str, result: Result<Vec<VersionInfo>, String>) {
    let Popup::Search(s) = &mut app.popup else {
        return;
    };
    let channel_versions = s.channel_versions(attr).to_vec();
    let search_result = s.results.iter().find(|r| r.name == attr).cloned();
    let Stage::Versions(picker) = &mut s.stage else {
        return;
    };
    if picker.package != attr {
        return;
    }
    match result {
        Ok(mut versions) => {
            if app.mode == Mode::Shell
                && let Some(rev) = app.system.as_ref().and_then(|i| i.nixpkgs_rev.as_deref())
            {
                match versions.iter_mut().find(|v| v.hash == rev) {
                    Some(v) => v.is_system = true,
                    None => versions.push(VersionInfo {
                        version: search_result
                            .as_ref()
                            .and_then(|r| r.versions.first())
                            .map(|v| v.version.clone())
                            .unwrap_or_else(|| "Unknown".to_string()),
                        hash: rev.to_string(),
                        date: String::new(),
                        is_unfree: search_result.as_ref().is_some_and(|r| r.is_unfree),
                        is_system: true,
                    }),
                }
            }
            picker.set_fetched(
                versions,
                flake_inputs(app.mode, &app.flake.inputs),
                &channel_versions,
            );
        }
        Err(e) => {
            picker.loading = false;
            crate::log::output("Nix Error", format!("Error fetching versions: {}", e));
            picker.error = Some(e);
        }
    }
}
