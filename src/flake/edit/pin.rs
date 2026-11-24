use super::ast::{is_with_list, outputs_pattern, replace_in_lists};
use super::inputs::add_input;
use super::packages::{drop_unused_input, inputs_bind};
use super::{Changes, EditError, Overlay, shell_lists};
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
    // Only `pkgs` must be in scope at the list, not `system`.
    let flake_src = sources
        .read(flake)
        .ok_or_else(|| EditError::Unreadable(flake.to_path_buf()))?;
    let bind = inputs_bind(&flake_src).unwrap_or_else(|| "inputs".to_string());
    let pinned_ref = format!(
        "{}.{}.legacyPackages.${{pkgs.stdenv.hostPlatform.system}}.{}",
        bind, pinned_input, attr
    );

    // The list item is replaced first so a package that isn't listed adds no input.
    let mut files = Overlay::new(sources);
    let shell = shell_lists(&files, flake, target)?;
    let candidates = [attr.to_string(), format!("pkgs.{}", attr), pkg.to_string()];
    let replaced = candidates.iter().find_map(|candidate| {
        let edits: Vec<_> = shell
            .files()
            .into_iter()
            .filter_map(|file| {
                let src = files.read(&file)?;
                let lists = shell.lists(&file, &src);
                let edited = replace_in_lists(&lists, candidate, |_| pinned_ref.clone(), &src);
                (edited != src).then_some((file, edited))
            })
            .collect();
        (!edits.is_empty()).then_some(edits)
    });
    for (file, edited) in replaced.ok_or_else(|| EditError::PackageNotFound(pkg.to_string()))? {
        files.set(&file, edited);
    }

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
pub(super) fn bind_inputs(src: &str) -> String {
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
    let Some((pinned_input, attr)) = parse::pinned_ref(pkg) else {
        return Err(match pkg.starts_with("inputs.") {
            true => EditError::InvalidPinnedRef(pkg.to_string()),
            false => EditError::NotPinned(pkg.to_string()),
        });
    };

    let mut files = Overlay::new(sources);
    let shell = shell_lists(&files, flake, target)
        .map_err(|_| EditError::PackageNotFound(pkg.to_string()))?;
    let mut found = false;
    for file in shell.files() {
        let src = files.get(&file)?;
        // A bare attribute only resolves inside `with pkgs;`.
        let edited = replace_in_lists(
            &shell.lists(&file, &src),
            pkg,
            |list| match is_with_list(list) {
                true => attr.to_string(),
                false => format!("pkgs.{}", attr),
            },
            &src,
        );
        found |= edited != src;
        files.set(&file, edited);
    }
    if !found {
        return Err(EditError::PackageNotFound(pkg.to_string()));
    }
    drop_unused_input(&mut files, flake, pinned_input)?;
    Ok(files.changes())
}
