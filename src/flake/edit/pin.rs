use super::ast::{is_with_list, outputs_pattern, package_lists, replace_in_lists};
use super::inputs::add_input;
use super::packages::drop_unused_input;
use super::{Changes, EditError, Overlay, find_shell, shell_node};
use crate::flake::locate::Sources;
use crate::flake::model::{Input, ShellTarget};
use crate::flake::parse;
use rnix::{Root, SyntaxKind};
use std::path::Path;

/// Pins `pkg` to the locked revision of its source input via a dedicated input.
pub fn pin_package(
    sources: &dyn Sources,
    flake: &Path,
    target: &ShellTarget,
    pkg: &str,
    inputs: &[Input],
) -> Result<Changes, EditError> {
    let (source, attr) = match parse::ref_source(pkg) {
        (Some("pkgs" | "self") | None, attr) => ("nixpkgs", attr),
        (Some(input), attr) => (input, attr),
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
    let pinned_ref = format!(
        "inputs.{}.legacyPackages.${{system}}.{}",
        pinned_input, attr
    );

    // The list item is replaced first so a package that isn't listed adds no input.
    let mut files = Overlay::new(sources);
    let location = find_shell(&files, flake, target)?;
    let src = files.get(&location.file)?;
    let lists = shell_node(&src, &location)
        .map(|set| package_lists(&set))
        .unwrap_or_default();
    let replaced = [attr.to_string(), format!("pkgs.{}", attr), pkg.to_string()]
        .iter()
        .map(|candidate| replace_in_lists(&lists, candidate, |_| pinned_ref.clone(), &src))
        .find(|replaced| *replaced != src)
        .ok_or_else(|| EditError::PackageNotFound(pkg.to_string()))?;
    files.set(&location.file, replaced);

    let flake_src = files.get(flake)?;
    files.set(
        flake,
        bind_inputs(&add_input(
            &flake_src,
            &pinned_input,
            &pinned_url(input, rev),
        )),
    );
    Ok(files.changes())
}

/// Binds the outputs pattern as `inputs@{ ... }` so pinned refs can reach their input.
fn bind_inputs(src: &str) -> String {
    let root = Root::parse(src).syntax();
    let Some(pattern) = outputs_pattern(&root) else {
        return src.to_string();
    };
    if pattern
        .children()
        .any(|c| c.kind() == SyntaxKind::NODE_PAT_BIND)
    {
        return src.to_string();
    }
    let mut result = src.to_string();
    result.insert_str(usize::from(pattern.text_range().start()), "inputs@");
    result
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
pub fn unpin_package(
    sources: &dyn Sources,
    flake: &Path,
    target: &ShellTarget,
    pkg: &str,
) -> Result<Changes, EditError> {
    if !pkg.starts_with("inputs.") {
        return Err(EditError::NotPinned(pkg.to_string()));
    }
    let parts: Vec<&str> = pkg.split('.').collect();
    if parts.len() < 5 {
        return Err(EditError::InvalidPinnedRef(pkg.to_string()));
    }
    let pinned_input = parts[1];
    let attr = parts[parts.len() - 1];

    let mut files = Overlay::new(sources);
    let location = find_shell(&files, flake, target)
        .map_err(|_| EditError::PackageNotFound(pkg.to_string()))?;
    let src = files.get(&location.file)?;
    let result = match shell_node(&src, &location) {
        // A bare attribute only resolves inside `with pkgs;`.
        Some(set) => replace_in_lists(
            &package_lists(&set),
            pkg,
            |list| match is_with_list(list) {
                true => attr.to_string(),
                false => format!("pkgs.{}", attr),
            },
            &src,
        ),
        None => src.clone(),
    };
    if result == src {
        return Err(EditError::PackageNotFound(pkg.to_string()));
    }
    files.set(&location.file, result);
    drop_unused_input(&mut files, flake, pinned_input)?;
    Ok(files.changes())
}
