use crate::nix::editor::inputs::{add_input, remove_input};
use crate::nix::editor::packages::{
    find_shell_node, get_shell_attr_set, replace_package_in_shell_attr_set,
};
use crate::nix::model::Input;
use crate::nix::parser::extract_outputs;
use crate::nix::query::extract_package_attribute_strings;
use color_eyre::Result;
use rnix::Root;
use std::path::Path;

pub fn pin_package(
    flake_path: &Path,
    system: &str,
    shell_name: &str,
    pkg_name: &str,
    inputs: &[Input],
) -> Result<()> {
    let content = std::fs::read_to_string(flake_path)?;

    let (source_input_name, actual_pkg_name) = if let Some((input, name)) = pkg_name.split_once('.')
    {
        if input == "pkgs" || input == "self" {
            ("nixpkgs".to_string(), name.to_string())
        } else {
            (input.to_string(), name.to_string())
        }
    } else {
        ("nixpkgs".to_string(), pkg_name.to_string())
    };

    let input = inputs
        .iter()
        .find(|i| i.name == source_input_name)
        .or_else(|| inputs.iter().find(|i| i.name == "nixpkgs"))
        .ok_or_else(|| color_eyre::eyre::eyre!("Could not find source input for {}", pkg_name))?;

    let rev = input.rev.as_ref().ok_or_else(|| {
        color_eyre::eyre::eyre!(
            "Input {} has no revision in lock file. Run 'nix flake lock' first.",
            input.name
        )
    })?;

    let pinned_input_name = format!("{}-{}", input.name, actual_pkg_name).replace('.', "-");

    let base_url = if let Some(pos) = input.url.find('?') {
        &input.url[..pos]
    } else {
        &input.url
    };

    let pinned_url = if input.url.contains("nixpkgs") {
        format!("github:nixos/nixpkgs/{}", rev)
    } else if base_url.starts_with("github:") {
        let parts: Vec<&str> = base_url.split('/').collect();
        if parts.len() >= 3 {
            format!("{}/{}/{}", parts[0], parts[1], rev)
        } else {
            format!("{}/{}", base_url, rev)
        }
    } else if base_url.contains('?') {
        format!("{}&rev={}", base_url, rev)
    } else {
        format!("{}?rev={}", base_url, rev)
    };

    let mut new_content = add_input(&content, &pinned_input_name, &pinned_url);

    let pinned_pkg_ref = format!(
        "inputs.{}.legacyPackages.${{system}}.{}",
        pinned_input_name, actual_pkg_name
    );

    let ast = Root::parse(&new_content);
    if let Some(shell_node) = find_shell_node(&ast.syntax(), system, shell_name) {
        if let Some(shell_attr_set) = get_shell_attr_set(&shell_node) {
            let candidates = vec![
                actual_pkg_name.to_string(),
                format!("pkgs.{}", actual_pkg_name),
                pkg_name.to_string(),
            ];

            for candidate in candidates {
                let replaced = replace_package_in_shell_attr_set(
                    &shell_attr_set,
                    &candidate,
                    &pinned_pkg_ref,
                    &new_content,
                );
                if replaced != new_content {
                    new_content = replaced;
                    break;
                }
            }
        }
    }

    if new_content != content {
        std::fs::write(flake_path, new_content)?;
        Ok(())
    } else {
        Err(color_eyre::eyre::eyre!(
            "Could not find package {} in flake.nix to pin",
            pkg_name
        ))
    }
}

pub fn unpin_package(
    flake_path: &Path,
    system: &str,
    shell_name: &str,
    pkg_name: &str,
) -> Result<()> {
    let content = std::fs::read_to_string(flake_path)?;

    if !pkg_name.starts_with("inputs.") {
        return Err(color_eyre::eyre::eyre!(
            "Package {} is not pinned",
            pkg_name
        ));
    }

    let parts: Vec<&str> = pkg_name.split('.').collect();
    if parts.len() < 5 {
        return Err(color_eyre::eyre::eyre!(
            "Invalid pinned package reference: {}",
            pkg_name
        ));
    }

    let pinned_input_name = parts[1];
    let actual_pkg_name = parts[parts.len() - 1];

    let unpinned_pkg_ref = actual_pkg_name;

    let ast = Root::parse(&content);
    let mut new_content = content.clone();

    if let Some(shell_node) = find_shell_node(&ast.syntax(), system, shell_name) {
        if let Some(shell_attr_set) = get_shell_attr_set(&shell_node) {
            new_content = replace_package_in_shell_attr_set(
                &shell_attr_set,
                pkg_name,
                unpinned_pkg_ref,
                &content,
            );
        }
    }

    if new_content != content {
        let configs = extract_outputs(&new_content);
        let mut used = false;
        for config in &configs {
            if let Some(config_content) = &config.content {
                let attrs = extract_package_attribute_strings(config_content);
                if attrs.iter().any(|a| {
                    a.starts_with(&format!("{}.", pinned_input_name)) || a == pinned_input_name
                }) {
                    used = true;
                    break;
                }
            }
        }

        if !used {
            if let Ok(content_no_input) = remove_input(&new_content, pinned_input_name) {
                new_content = content_no_input;
            }
        }

        std::fs::write(flake_path, new_content)?;
        Ok(())
    } else {
        Err(color_eyre::eyre::eyre!(
            "Could not find pinned package {} in flake.nix",
            pkg_name
        ))
    }
}
