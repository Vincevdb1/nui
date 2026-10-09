use super::EditError;
use super::ast::{package_lists, replace_in_lists, shell_set};
use super::inputs::{add_input, remove_input};
use super::packages::input_is_used;
use crate::flake::model::{Input, ShellTarget};
use rnix::Root;

/// Pins `pkg` to the locked revision of its source input via a dedicated input.
pub fn pin_package(
    src: &str,
    target: &ShellTarget,
    pkg: &str,
    inputs: &[Input],
) -> Result<String, EditError> {
    let (source, attr) = match pkg.split_once('.') {
        Some(("pkgs" | "self", attr)) => ("nixpkgs", attr),
        Some((input, attr)) => (input, attr),
        None => ("nixpkgs", pkg),
    };

    let input = inputs
        .iter()
        .find(|i| i.name == source)
        .or_else(|| inputs.iter().find(|i| i.name == "nixpkgs"))
        .ok_or_else(|| EditError::NoSourceInput(pkg.to_string()))?;
    let rev = input
        .rev
        .as_ref()
        .ok_or_else(|| EditError::Unlocked(input.name.clone()))?;

    let pinned_input = format!("{}-{}", input.name, attr).replace('.', "-");
    let mut result = add_input(src, &pinned_input, &pinned_url(input, rev));

    let pinned_ref = format!(
        "inputs.{}.legacyPackages.${{system}}.{}",
        pinned_input, attr
    );
    let root = Root::parse(&result).syntax();
    if let Some(set) = shell_set(&root, target) {
        let lists = package_lists(&set);
        for candidate in [attr.to_string(), format!("pkgs.{}", attr), pkg.to_string()] {
            let replaced = replace_in_lists(&lists, &candidate, &pinned_ref, &result);
            if replaced != result {
                result = replaced;
                break;
            }
        }
    }

    if result == src {
        return Err(EditError::PackageNotFound(pkg.to_string()));
    }
    Ok(result)
}

fn pinned_url(input: &Input, rev: &str) -> String {
    let base = input.url.split('?').next().unwrap_or(&input.url);
    if input.is_nixpkgs() {
        format!("github:nixos/nixpkgs/{}", rev)
    } else if base.starts_with("github:") {
        let parts: Vec<&str> = base.split('/').collect();
        if parts.len() >= 3 {
            format!("{}/{}/{}", parts[0], parts[1], rev)
        } else {
            format!("{}/{}", base, rev)
        }
    } else {
        format!("{}?rev={}", base, rev)
    }
}

/// Reverts a pinned `inputs.<name>.legacyPackages.${system}.<attr>` reference.
pub fn unpin_package(src: &str, target: &ShellTarget, pkg: &str) -> Result<String, EditError> {
    if !pkg.starts_with("inputs.") {
        return Err(EditError::NotPinned(pkg.to_string()));
    }
    let parts: Vec<&str> = pkg.split('.').collect();
    if parts.len() < 5 {
        return Err(EditError::InvalidPinnedRef(pkg.to_string()));
    }
    let pinned_input = parts[1];
    let attr = parts[parts.len() - 1];

    let root = Root::parse(src).syntax();
    let mut result = match shell_set(&root, target) {
        Some(set) => replace_in_lists(&package_lists(&set), pkg, attr, src),
        None => src.to_string(),
    };
    if result == src {
        return Err(EditError::PackageNotFound(pkg.to_string()));
    }
    if !input_is_used(&result, pinned_input) {
        result = remove_input(&result, pinned_input);
    }
    Ok(result)
}
