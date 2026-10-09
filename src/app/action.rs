use super::effect::Generation;
use crate::backend::github::Branch;
use crate::backend::nix::Metadata;
use crate::backend::nxv::VersionInfo;
use crate::backend::search::SearchResult;
use crate::backend::system::{SystemInfo, Tools};
use crate::backend::templates::Template;
use crate::flake::{Input, Output, Package};
use crate::log::LogEntry;
use crate::pkgref::PackageRef;
use ratatui::crossterm::event::KeyEvent;

pub enum Action {
    Key(KeyEvent),
    Tick,
    Log(LogEntry),

    SystemProbed(SystemInfo, Tools),
    FlakeLoaded(Vec<Input>, Vec<Output>),
    FlakeChanged,
    PackagesLoaded(Generation, Result<Vec<Package>, String>),
    LatestVersion {
        attr: String,
        version: String,
    },

    SearchFinished(Generation, Result<Vec<SearchResult>, String>),
    LockedVersion {
        generation: Generation,
        attr: String,
        channel: String,
        version: String,
    },
    VersionsLoaded(String, Result<Vec<VersionInfo>, String>),

    ShellMetadata(PackageRef, Metadata),
    ShellTemplateLoaded(Vec<String>),
    BranchesLoaded(Result<Vec<Branch>, String>),
    TemplatesListed(Vec<Template>),
    NxvProgress(Option<String>),
}
