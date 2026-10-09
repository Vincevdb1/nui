use crate::backend::system::SystemInfo;
use crate::pkgref::PackageRef;
use color_eyre::owo_colors::OwoColorize;
use std::process::Command;

/// Replaces the TUI with `nix shell` for the chosen packages.
pub fn launch(packages: &[PackageRef], system: &SystemInfo) -> std::io::Result<()> {
    let installables: Vec<String> = packages.iter().map(|p| p.installable(system)).collect();
    println!(
        "Opening shell with packages: {}",
        installables.join(", ").cyan()
    );

    let names: Vec<String> = packages.iter().map(PackageRef::short_name).collect();
    Command::new("nix")
        .arg("shell")
        .args(&installables)
        .arg("--impure")
        .env("name", format!("nui-shell:{}-env", names.join(":")))
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .status()?;
    Ok(())
}
