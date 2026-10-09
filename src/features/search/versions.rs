use crate::backend::nxv::VersionInfo;
use crate::backend::search::ChannelVersion;
use crate::flake::Input;
use crate::ui::widgets::Selectable;
use ratatui::widgets::ListState;
use std::cmp::Ordering;

pub enum VersionItem {
    /// The package as provided by one of the flake's inputs.
    Input {
        input: String,
        channel: String,
        version: String,
        rev: String,
    },
    Version(VersionInfo),
}

impl VersionItem {
    pub fn version(&self) -> &str {
        match self {
            Self::Input { version, .. } => version,
            Self::Version(v) => &v.version,
        }
    }
}

pub struct VersionPicker {
    pub package: String,
    pub description: String,
    pub is_unfree: bool,
    /// The shell package being swapped for another version.
    pub replacing: Option<usize>,
    pub loading: bool,
    pub error: Option<String>,
    fetched: Vec<VersionInfo>,
    /// What the list shows, newest first; Enter picks from the same list.
    pub items: Vec<VersionItem>,
    pub list: ListState,
}

impl VersionPicker {
    pub fn new(
        package: String,
        description: String,
        is_unfree: bool,
        replacing: Option<usize>,
    ) -> Self {
        Self {
            package,
            description,
            is_unfree,
            replacing,
            loading: true,
            error: None,
            fetched: Vec::new(),
            items: Vec::new(),
            list: ListState::default(),
        }
    }

    pub fn set_fetched(
        &mut self,
        fetched: Vec<VersionInfo>,
        inputs: &[Input],
        channel_versions: &[ChannelVersion],
    ) {
        self.loading = false;
        self.fetched = fetched;
        self.rebuild(inputs, channel_versions);
        if !self.items.is_empty() {
            self.list.select(Some(0));
        }
    }

    /// Lists the inputs that carry the package (flake mode only) and the fetched history.
    pub fn rebuild(&mut self, inputs: &[Input], channel_versions: &[ChannelVersion]) {
        let from_inputs = inputs.iter().filter_map(|input| {
            let channel = input.channel();
            let cv = channel_versions.iter().find(|v| v.channel == channel)?;
            Some(VersionItem::Input {
                input: input.name.clone(),
                version: cv
                    .locked_version
                    .clone()
                    .unwrap_or_else(|| cv.version.clone()),
                rev: input.rev.clone().unwrap_or_else(|| "-------".to_string()),
                channel,
            })
        });
        let history = self.fetched.iter().cloned().map(VersionItem::Version);
        self.items = from_inputs.chain(history).collect();
        self.items
            .sort_by(|a, b| compare_versions(b.version(), a.version()));
    }

    pub fn step(&mut self, forward: bool) {
        self.list.step(self.items.len(), forward);
    }

    pub fn selected(&self) -> Option<&VersionItem> {
        self.items.get(self.list.selected()?)
    }
}

fn compare_versions(a: &str, b: &str) -> Ordering {
    let a_parts: Vec<&str> = a.split(|c: char| !c.is_alphanumeric()).collect();
    let b_parts: Vec<&str> = b.split(|c: char| !c.is_alphanumeric()).collect();
    for (a, b) in a_parts.iter().zip(&b_parts) {
        let ordering = match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(a), Ok(b)) => a.cmp(&b),
            _ => a.cmp(b),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    a_parts.len().cmp(&b_parts.len())
}
