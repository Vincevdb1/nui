use color_eyre::eyre::Result;
use ratatui::crossterm::event;

mod action;
mod app;
mod components;
mod context;
pub mod handlers;
mod nix;
pub mod services;
mod state;
mod tui;
mod ui;

use action::Action;
use app::App;

pub use components::command_log::{command_log, log_action, log_output};

fn main() -> Result<()> {
    color_eyre::install()?;
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let (mode, shell_packages) = if args.iter().any(|arg| arg == "shell") {
        (crate::state::Mode::Shell, Vec::new())
    } else {
        (crate::state::Mode::Flake, Vec::new())
    };

    if let Some(config_dir) = dirs::config_dir() {
        let template_dir = config_dir.join("nui").join("templates");
        if !template_dir.exists() {
            let _ = std::fs::create_dir_all(&template_dir);
        }
    }

    let mut terminal = tui::init()?;
    let mut app = App::new(mode.clone(), shell_packages);

    if !std::path::Path::new("flake.nix").exists() && mode == crate::state::Mode::Flake {
        app.ui.show_templates = true;
        if !app.ui.templates.is_empty() {
            app.ui.template_list_state.select(Some(2));
        }
    }

    let result = run(&mut terminal, &mut app);

    tui::restore()?;

    if let Ok(Some(pkgs)) = result {
        if !pkgs.is_empty() {
            let system_nixpkgs_path = app.domain.system_nixpkgs_path.clone();
            let system_version = app.domain.system_nixpkgs_version.clone();

            let normalized_pkgs: Vec<String> = pkgs
                .iter()
                .map(|pkg| {
                    let pkg_ref = if let Some((path, _version)) = pkg.rsplit_once('@') {
                        path
                    } else {
                        pkg
                    };

                    if pkg_ref.starts_with("system/") {
                        if let Some((_, suffix)) = pkg_ref.split_once('#') {
                            if let Some(path) = &system_nixpkgs_path {
                                if std::path::Path::new(path).exists() {
                                    return format!("path:{}#{}", path, suffix);
                                }
                            }

                            if let Some(version) = &system_version {
                                let channel =
                                    if version.contains("pre") || version.contains("unstable") {
                                        "nixpkgs-unstable"
                                    } else {
                                        let parts: Vec<&str> = version.split('.').collect();
                                        if parts.len() >= 2 {
                                            &format!("nixos-{}", &parts[..2].join("."))
                                        } else {
                                            "nixpkgs-unstable"
                                        }
                                    };
                                return format!("nixpkgs/{}#{}", channel, suffix);
                            }
                        }

                        let hash_part = &pkg_ref[7..];
                        format!("github:NixOS/nixpkgs/{}", hash_part)
                    } else if pkg_ref.starts_with("nixpkgs/") {
                        let hash_part = &pkg_ref[8..];
                        if hash_part.contains('#') {
                            let (hash, _pkg) = hash_part.split_once('#').unwrap();
                            if hash.len() != 40 && !hash.contains('.') {
                                return format!("nixpkgs#{}", hash_part);
                            }
                        }
                        format!("github:NixOS/nixpkgs/{}", hash_part)
                    } else if pkg_ref.contains('#') {
                        pkg_ref.to_string()
                    } else {
                        format!("nixpkgs#{}", pkg_ref)
                    }
                })
                .collect();

            use color_eyre::owo_colors::OwoColorize;
            println!(
                "Opening shell with packages: {}",
                normalized_pkgs.join(", ").cyan()
            );

            let mut cmd = std::process::Command::new("nix");
            cmd.arg("shell");
            for pkg in normalized_pkgs {
                cmd.arg(pkg);
            }
            cmd.arg("--impure");

            let env_name = {
                let shortened_packages: Vec<String> = pkgs
                    .iter()
                    .map(|pkg| {
                        let mut display_pkg = pkg.clone();
                        if let Some(hash_idx) = display_pkg.find("nixpkgs/") {
                            if let Some(hash_end) = display_pkg[hash_idx + 8..].find('#') {
                                let hash = &display_pkg[hash_idx + 8..hash_idx + 8 + hash_end];
                                if hash.len() > 7 {
                                    display_pkg = format!(
                                        "{}{}{}",
                                        &display_pkg[..hash_idx + 8],
                                        &hash[..7],
                                        &display_pkg[hash_idx + 8 + hash_end..]
                                    );
                                }
                            }
                        } else if let Some(hash_idx) = display_pkg.find("system/") {
                            if let Some(hash_end) = display_pkg[hash_idx + 7..].find('#') {
                                let hash = &display_pkg[hash_idx + 7..hash_idx + 7 + hash_end];
                                let short_hash = if hash.len() > 7 { &hash[..7] } else { hash };
                                display_pkg = format!(
                                    "{}#{}",
                                    short_hash,
                                    &display_pkg[hash_idx + 7 + hash_end + 1..]
                                );
                            }
                        }
                        display_pkg
                    })
                    .collect();
                format!("nui-shell:{}-env", shortened_packages.join(":"))
            };

            cmd.env("name", env_name);
            cmd.env("NIXPKGS_ALLOW_UNFREE", "1");
            cmd.status()?;
        }
    }

    Ok(())
}

fn run(terminal: &mut tui::Tui, app: &mut App) -> Result<Option<Vec<String>>> {
    let mut shell_packages = None;
    while !app.should_quit {
        terminal.draw(|frame| ui::render(app, frame))?;

        // Drain every keystroke that is already queued before rendering again, so a burst
        // of typing is applied in a single frame instead of one character per redraw.
        if event::poll(std::time::Duration::from_millis(16))? {
            let mut budget = 64;
            while budget > 0 && event::poll(std::time::Duration::ZERO)? {
                budget -= 1;
                if let Some(action) = ui::events::map_event(app, event::read()?) {
                    if let Action::StartShell(ref pkgs) = action {
                        shell_packages = Some(pkgs.clone());
                    }
                    app.update(action);
                }
            }
        }
        app.update(Action::Tick);
    }
    Ok(shell_packages)
}
