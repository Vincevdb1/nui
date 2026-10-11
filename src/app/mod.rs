pub mod action;
pub mod effect;
pub mod update;

pub use action::Action;
pub use effect::{Effect, Executor, Generation};

use crate::backend::search::SearchResult;
use crate::backend::system::{SystemInfo, Tools};
use crate::backend::templates::{self, Template};
use crate::features::{
    add_input::AddInput,
    flake::{self, FlakeState},
    log::LogState,
    search::Search,
    shell::{ShellState, flakes::FlakeList},
    templates::{SaveTemplate, TemplatePicker},
};
use crate::pkgref::PackageRef;
use throbber_widgets_tui::ThrobberState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Flake,
    Shell,
}

/// The flake-mode panel that receives navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Title,
    Context,
    Inputs,
    Outputs,
    Log,
}

impl Focus {
    const CYCLE: [Focus; 4] = [Focus::Title, Focus::Context, Focus::Inputs, Focus::Outputs];

    pub fn from_digit(c: char) -> Option<Self> {
        Some(match c {
            '1' => Self::Title,
            '2' => Self::Context,
            '3' => Self::Inputs,
            '4' => Self::Outputs,
            '5' => Self::Log,
            _ => return None,
        })
    }

    /// Tab cycles the first four panels; the log is reached with `5` only.
    pub fn cycle(self, forward: bool) -> Self {
        let i = Self::CYCLE.iter().position(|f| *f == self);
        let len = Self::CYCLE.len();
        let next = match (i, forward) {
            (None, _) => 0,
            (Some(i), true) => (i + 1) % len,
            (Some(i), false) => (i + len - 1) % len,
        };
        Self::CYCLE[next]
    }
}

#[derive(Default)]
pub enum Popup {
    #[default]
    None,
    Help,
    Templates(TemplatePicker),
    ConfirmTemplate(String),
    SaveTemplate(SaveTemplate),
    AddInput(AddInput),
    Search(Search),
    Details(SearchResult),
    ShellFlakes(FlakeList),
}

pub enum Exit {
    Quit,
    Shell(Vec<PackageRef>),
}

pub struct App {
    pub mode: Mode,
    pub focus: Focus,
    pub popup: Popup,
    pub flake: FlakeState,
    pub shell: ShellState,
    pub log: LogState,
    pub system: Option<SystemInfo>,
    pub tools: Tools,
    pub templates: Vec<Template>,
    pub nxv_progress: Option<String>,
    pub throbber: ThrobberState,
    pub search_generation: Generation,
    pub exit: Option<Exit>,
    pub effects: Vec<Effect>,
}

impl App {
    pub fn new(mode: Mode) -> Self {
        let mut app = Self {
            mode,
            focus: if mode == Mode::Shell {
                Focus::Title
            } else {
                Focus::Context
            },
            popup: Popup::None,
            flake: FlakeState::default(),
            shell: ShellState::default(),
            log: LogState::default(),
            system: None,
            tools: Tools::default(),
            templates: templates::list(),
            nxv_progress: None,
            throbber: ThrobberState::default(),
            search_generation: Generation::default(),
            exit: None,
            effects: vec![Effect::ProbeSystem],
        };
        crate::log::info("Initializing NUI Application...");

        match mode {
            Mode::Flake => {
                flake::load(&mut app);
                if !std::path::Path::new("flake.nix").exists() {
                    app.popup = Popup::Templates(TemplatePicker::new(&app.templates));
                }
            }
            Mode::Shell => crate::features::search::open(&mut app),
        }
        app
    }

    pub fn system_version(&self) -> Option<&str> {
        self.system.as_ref()?.nixpkgs_version.as_deref()
    }

    pub fn system_rev(&self) -> Option<&str> {
        self.system.as_ref()?.nixpkgs_rev.as_deref()
    }
}
