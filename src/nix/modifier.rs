use crate::nix::editor;
use crate::nix::model::{Input, Output};
use crate::nix::traits::NixEditor;
use color_eyre::Result;
use std::path::Path;

pub struct FlakeEditor;

impl NixEditor for FlakeEditor {
    type Error = color_eyre::Report;

    fn add_package(flake_path: &Path, output: &Output, package: &str) -> Result<(), Self::Error> {
        let parts: Vec<&str> = output.path.split('.').collect();
        let system = parts.get(0).copied().unwrap_or("x86_64-linux");
        let shell_name = parts.get(1).copied().unwrap_or("default");
        editor::add_package(flake_path, system, shell_name, package)
    }

    fn remove_package(
        flake_path: &Path,
        output: &Output,
        package: &str,
    ) -> Result<(), Self::Error> {
        let parts: Vec<&str> = output.path.split('.').collect();
        let system = parts.get(0).copied().unwrap_or("x86_64-linux");
        let shell_name = parts.get(1).copied().unwrap_or("default");
        editor::remove_package(flake_path, system, shell_name, package)
    }

    fn add_input(flake_path: &Path, input: &Input) -> Result<(), Self::Error> {
        let content = std::fs::read_to_string(flake_path)?;
        let new_content = editor::add_input(&content, &input.name, &input.url);
        std::fs::write(flake_path, new_content)?;
        Ok(())
    }

    fn remove_input(flake_path: &Path, input_name: &str) -> Result<(), Self::Error> {
        let content = std::fs::read_to_string(flake_path)?;
        let new_content = editor::remove_input(&content, input_name)?;
        std::fs::write(flake_path, new_content)?;
        Ok(())
    }
}
