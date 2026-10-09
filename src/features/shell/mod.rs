pub mod keys;
pub mod view;

use crate::app::{App, Effect};
use crate::backend::nix::Metadata;
use crate::pkgref::{PackageRef, Source};
use ratatui::widgets::TableState;
use std::collections::HashSet;

pub struct ShellPackage {
    pub reference: PackageRef,
    pub description: String,
    pub is_unfree: bool,
}

#[derive(Default)]
pub struct ShellState {
    pub packages: Vec<ShellPackage>,
    pub table: TableState,
    pub marked: HashSet<usize>,
}

impl ShellState {
    pub fn contains(&self, reference: &PackageRef) -> bool {
        self.packages.iter().any(|p| p.reference == *reference)
    }

    pub fn selected(&self) -> Option<usize> {
        self.table.selected().filter(|i| *i < self.packages.len())
    }

    /// Adds the package unless it is already present; returns whether it was added.
    pub fn add(&mut self, package: ShellPackage) -> bool {
        if self.contains(&package.reference) {
            return false;
        }
        self.packages.push(package);
        true
    }

    pub fn references(&self) -> Vec<PackageRef> {
        self.packages.iter().map(|p| p.reference.clone()).collect()
    }

    pub fn apply_metadata(&mut self, reference: &PackageRef, metadata: Metadata) {
        if let Some(pkg) = self.packages.iter_mut().find(|p| p.reference == *reference) {
            pkg.reference.version = Some(metadata.version);
            pkg.description = metadata.description;
        }
    }

    pub fn remove(&mut self, indices: &[usize]) {
        let mut indices = indices.to_vec();
        indices.sort_unstable_by(|a, b| b.cmp(a));
        indices.dedup();
        for i in &indices {
            if *i < self.packages.len() {
                self.packages.remove(*i);
            }
        }
        self.marked.clear();
        let len = self.packages.len();
        self.table.select(match indices.as_slice() {
            _ if len == 0 => None,
            [single] => Some((*single).min(len - 1)),
            _ => Some(0),
        });
    }
}

/// Adds template packages from the system nixpkgs when it is known.
pub fn add_from_template(app: &mut App, attrs: Vec<String>) {
    let source = match app.system_rev() {
        Some(rev) => Source::System(rev.to_string()),
        None => Source::Registry,
    };
    for attr in attrs {
        let reference = PackageRef::new(source.clone(), attr, None);
        let added = app.shell.add(ShellPackage {
            reference: reference.clone(),
            description: String::new(),
            is_unfree: false,
        });
        if added {
            crate::log::output("Debug", format!("Adding to shell: {}", reference));
            app.effects.push(Effect::ShellMetadata(reference));
        }
    }
}
