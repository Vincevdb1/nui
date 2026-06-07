use super::NixService;
use crate::action::Action;
use crate::nix::modifier::FlakeEditor;
use crate::nix::parser::{extract_inputs, fetch_outputs};
use crate::nix::traits::NixEditor;
use crate::nix::{Input, Output};

impl NixService {
    pub fn refresh_context(&self, flake_path: std::path::PathBuf) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let content = std::fs::read_to_string(&flake_path).unwrap_or_default();
            let lock_path = flake_path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("flake.lock");
            let lock_content = std::fs::read_to_string(lock_path).ok();
            let inputs = extract_inputs(&content, lock_content.as_deref());

            let outputs =
                match fetch_outputs(flake_path.parent().unwrap_or(std::path::Path::new("."))) {
                    Ok(outputs) => outputs,
                    Err(e) => {
                        crate::log_output("Nix Error", format!("Failed to fetch outputs: {}", e));
                        Vec::new()
                    }
                };

            let _ = tx.send(Action::SetContextData(inputs, outputs));
        });
    }

    pub fn remove_input(&self, flake_path: std::path::PathBuf, input_name: String) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            match FlakeEditor::remove_input(&flake_path, &input_name) {
                Ok(_) => {
                    crate::log_output("Success", format!("Removed input: {}", input_name));
                }
                Err(e) => {
                    crate::log_output("Error", format!("Failed to remove input: {}", e));
                }
            }
            let _ = tx.send(Action::RefreshContext);
        });
    }

    pub fn remove_package(&self, flake_path: std::path::PathBuf, output: Output, pkg_name: String) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = FlakeEditor::remove_package(&flake_path, &output, &pkg_name) {
                crate::log_output("Error", format!("Failed to remove package: {}", e));
            } else {
                crate::log_output(
                    "Success",
                    format!("Removed {} from {}", pkg_name, output.path),
                );
            }
            let _ = tx.send(Action::RefreshContext);
        });
    }

    pub fn remove_packages(
        &self,
        flake_path: std::path::PathBuf,
        system: String,
        shell_name: String,
        pkg_names: Vec<String>,
    ) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            if let Err(e) =
                crate::nix::editor::remove_packages(&flake_path, &system, &shell_name, &pkg_names)
            {
                crate::log_output("Error", format!("Failed to remove packages: {}", e));
            } else {
                crate::log_output(
                    "Success",
                    format!("Removed {} packages from {}", pkg_names.len(), shell_name),
                );
            }
            let _ = tx.send(Action::RefreshContext);
        });
    }

    pub fn add_package(
        &self,
        flake_path: std::path::PathBuf,
        output: Output,
        package_name: String,
    ) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = FlakeEditor::add_package(&flake_path, &output, &package_name) {
                crate::log_output("Error", format!("Failed to add package: {}", e));
            } else {
                crate::log_output(
                    "Success",
                    format!("Added {} to {}", package_name, output.path),
                );
            }
            let _ = tx.send(Action::RefreshContext);
        });
    }

    pub fn add_input_and_package(
        &self,
        flake_path: std::path::PathBuf,
        name: String,
        url: String,
        pkg_name: Option<String>,
        output: Option<Output>,
    ) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let content = std::fs::read_to_string(&flake_path).unwrap_or_default();
            let new_content = crate::nix::editor::add_input(&content, &name, &url);

            if let Some(pkg_name) = pkg_name {
                if let Some(output) = output {
                    let prefixed_pkg = format!("{}.legacyPackages.${{system}}.{}", name, pkg_name);

                    if let Err(e) = std::fs::write(&flake_path, &new_content) {
                        crate::log_output("Error", format!("Failed to write flake.nix: {}", e));
                        let _ = tx.send(Action::RefreshContext);
                        return;
                    }

                    if let Err(e) = FlakeEditor::add_package(&flake_path, &output, &prefixed_pkg) {
                        crate::log_output("Error", format!("Failed to add package: {}", e));
                    } else {
                        crate::log_output(
                            "Success",
                            format!("Added {} to {}", prefixed_pkg, output.path),
                        );
                    }
                }
            } else {
                if let Err(e) = std::fs::write(&flake_path, new_content) {
                    crate::log_output("Error", format!("Failed to write flake.nix: {}", e));
                } else {
                    crate::log_output("Success", format!("Added input: {}", name));
                }
            }
            let _ = tx.send(Action::RefreshContext);
        });
    }

    pub fn pin_package(
        &self,
        flake_path: std::path::PathBuf,
        system: String,
        shell_name: String,
        pkg_name: String,
        inputs: Vec<Input>,
    ) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            match crate::nix::editor::pin_package(
                &flake_path,
                &system,
                &shell_name,
                &pkg_name,
                &inputs,
            ) {
                Ok(_) => {
                    crate::log_output("Success", format!("Pinned package {}", pkg_name));
                    let _ = tx.send(Action::RefreshContext);
                }
                Err(e) => {
                    crate::log_output("Error", format!("Failed to pin {}: {}", pkg_name, e));
                }
            }
        });
    }

    pub fn unpin_package(
        &self,
        flake_path: std::path::PathBuf,
        system: String,
        shell_name: String,
        pkg_name: String,
    ) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            match crate::nix::editor::unpin_package(&flake_path, &system, &shell_name, &pkg_name) {
                Ok(_) => {
                    crate::log_output("Success", format!("Unpinned package {}", pkg_name));
                    let _ = tx.send(Action::RefreshContext);
                }
                Err(e) => {
                    crate::log_output("Error", format!("Failed to unpin {}: {}", pkg_name, e));
                }
            }
        });
    }

    pub fn start_nxv_update_check(&self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            use std::io::{BufReader, Read};
            use std::process::{Command, Stdio};

            let mut child = match Command::new("script")
                .args(["-q", "-c", "nxv update", "/dev/null"])
                .stdout(Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    crate::log_output("NXV Error", format!("Failed to spawn nxv update: {}", e));
                    return;
                }
            };

            let stdout = child.stdout.take().unwrap();
            let mut reader = BufReader::new(stdout);
            let mut buffer = Vec::new();
            let mut b = [0u8; 1];

            fn strip_ansi(s: &str) -> String {
                let mut result = String::new();
                let mut iter = s.chars();
                while let Some(c) = iter.next() {
                    if c == '\x1b' {
                        if let Some('[') = iter.next() {
                            while let Some(c2) = iter.next() {
                                if (0x40..=0x7e).contains(&(c2 as u8)) {
                                    break;
                                }
                            }
                        }
                    } else {
                        result.push(c);
                    }
                }
                result
            }

            while reader.read_exact(&mut b).is_ok() {
                if b[0] == b'\n' || b[0] == b'\r' {
                    let line = String::from_utf8_lossy(&buffer);
                    let clean_line = strip_ansi(&line);
                    let trimmed = clean_line.trim();
                    if !trimmed.is_empty() {
                        let display = if let Some(start) = trimmed.find('[') {
                            if let Some(end) = trimmed.find(']') {
                                let stats = trimmed[end + 1..].trim();
                                let bar_content = &trimmed[start + 1..end];
                                let filled_count = bar_content
                                    .chars()
                                    .filter(|&c| !c.is_whitespace() && c != '-')
                                    .count();
                                let total_chars = bar_content.chars().count();
                                let ratio = if total_chars > 0 {
                                    filled_count as f32 / total_chars as f32
                                } else {
                                    0.0
                                };
                                let filled_segments = (ratio * 10.0).round() as usize;
                                let bar = format!(
                                    "[{}{}]",
                                    "█".repeat(filled_segments),
                                    " ".repeat(10 - filled_segments)
                                );
                                format!("{} {}", bar, stats)
                            } else {
                                trimmed[start..].to_string()
                            }
                        } else {
                            trimmed.to_string()
                        };
                        let _ = tx.send(Action::UpdateNxvProgress(Some(display)));
                    }
                    buffer.clear();
                } else {
                    buffer.push(b[0]);
                }
            }

            let status = child.wait();
            let _ = tx.send(Action::UpdateNxvProgress(None));

            if let Ok(status) = status {
                if status.success() {
                    let _ = tx.send(Action::Log(crate::components::command_log::LogEntry::Info(
                        "NXV index updated successfully".to_string(),
                    )));
                } else {
                    let _ = tx.send(Action::Log(crate::components::command_log::LogEntry::Info(
                        format!("NXV update failed with status: {}", status),
                    )));
                }
            }
        });
    }
}
