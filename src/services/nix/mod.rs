use crate::action::Action;
use crate::nix::parser::{extract_inputs, fetch_outputs};
use crate::nix::{Input, Output};
use crate::state::domain::ChildRegistry;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::Sender;

pub mod edit;
pub mod search;
pub mod search_sort;
pub mod templates;

/// NixService coordinates domain modules and handles file IO and background threading.
pub struct NixService {
    pub(crate) tx: Sender<Action>,
    /// Child processes of the in-flight package search, so a newer query can kill them.
    pub(crate) search_children: ChildRegistry,
    /// Id of the newest package search; worker threads compare against it before reporting.
    pub(crate) current_search_id: Arc<AtomicUsize>,
}

impl NixService {
    pub fn new(tx: Sender<Action>) -> Self {
        Self {
            tx,
            search_children: ChildRegistry::default(),
            current_search_id: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Registry of the current search's child processes, for cancelling from elsewhere.
    pub fn search_children(&self) -> ChildRegistry {
        Arc::clone(&self.search_children)
    }

    /// True while `search_id` is still the newest search that was started.
    pub fn is_current_search(&self, search_id: usize) -> bool {
        self.current_search_id
            .load(std::sync::atomic::Ordering::SeqCst)
            == search_id
    }

    pub fn get_initial_context(&self) -> (Vec<crate::context::NixFile>, Vec<Input>, Vec<Output>) {
        let nix_files = crate::context::find_nix_files();
        crate::log_output("Filesystem", format!("Found {} nix files", nix_files.len()));

        let (inputs, outputs) = if let Some(file) = nix_files.first() {
            let flake_content = std::fs::read_to_string(&file.path).unwrap_or_default();
            let lock_path = file
                .path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("flake.lock");
            let lock_content = std::fs::read_to_string(lock_path).ok();
            (
                extract_inputs(&flake_content, lock_content.as_deref()),
                fetch_outputs(file.path.parent().unwrap_or(std::path::Path::new(".")))
                    .unwrap_or_default(),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        (nix_files, inputs, outputs)
    }

    pub fn get_system_info(&self) -> (Option<String>, Option<String>, Option<String>) {
        let version = std::process::Command::new("nix-instantiate")
            .args([
                "--eval",
                "-E",
                "(import <nixpkgs> {}).lib.version",
                "--json",
            ])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    serde_json::from_slice::<String>(&o.stdout).ok()
                } else {
                    None
                }
            });

        let path = std::process::Command::new("nix")
            .args(["eval", "--raw", "--impure", "--expr", "toString <nixpkgs>"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() { None } else { Some(s) }
                } else {
                    None
                }
            });

        let hash = std::process::Command::new("nixos-version")
            .arg("--revision")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.len() >= 7 { Some(s) } else { None }
                } else {
                    None
                }
            })
            .or_else(|| {
                // Try to find it in the store path name if it's not a standard revision
                std::process::Command::new("nix")
                    .args([
                        "eval",
                        "--raw",
                        "--impure",
                        "--expr",
                        "builtins.substring 0 32 (builtins.baseNameOf (toString <nixpkgs>))",
                    ])
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                            if s.is_empty() { None } else { Some(s) }
                        } else {
                            None
                        }
                    })
            });

        (version, path, hash)
    }

    pub fn get_tool_versions(&self) -> (Option<String>, Option<String>) {
        (
            crate::state::domain::get_tool_version("nix-search"),
            crate::state::domain::get_tool_version("nxv"),
        )
    }
}
